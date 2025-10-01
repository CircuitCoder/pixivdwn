use clap::{Args, Subcommand};
use futures::StreamExt;

use crate::{
    data::fanbox::FanboxRequest,
    util::{DatabasePathFormat, DownloadResult, TerminationCondition},
};

#[derive(Args)]
pub struct Fanbox {
    #[command(subcommand)]
    cmd: FanboxCmd,
}

#[derive(clap::ValueEnum, Clone, Copy)]
pub enum FanboxDownloadType {
    File,
    Image,
}

#[derive(Args)]
pub struct FanboxSyncArgs {
    /// ID of the Fanbox creator. If not spcified, sync all supported creators
    creator: Option<String>,

    #[arg(alias="term", long, value_enum, default_value_t = TerminationCondition::UntilEnd)]
    /// Termination condition (alias: --term)
    termination: TerminationCondition,

    /// Skip pages. Can only be used when `creator` is specified
    #[arg(long, requires("creator"))]
    skip_pages: Option<usize>,

    /// Skip failed posts instead of aborting
    #[arg(long, default_value_t = false)]
    skip_failed: bool,

    /// Maximum number of retries for post fetch
    #[arg(short, long, default_value_t = 0)]
    retries: usize,

    /// Exponential backoff base for retries
    #[arg(short, long)]
    retry_backoff: Option<usize>,
}

impl FanboxSyncArgs {
    async fn sync(
        &self,
        session: &crate::config::Session,
        creator: &str,
    ) -> anyhow::Result<()> {
        let mut posts = Box::pin(crate::data::fanbox::fetch_author_posts(
            session, creator, self.skip_pages.unwrap_or(0),
        ));
        'post: while let Some(post) = posts.next().await.transpose()? {
            let last_updated = crate::db::query_fanbox_post_updated_datetime(post.id).await?;
            if let Some(last_updated) = last_updated
                && last_updated >= post.updated_datetime
            {
                if last_updated > post.updated_datetime {
                    tracing::warn!(
                        "Post {} updated_datetime went backwards: was {}, now {}",
                        post.id,
                        last_updated,
                        post.updated_datetime
                    );
                } else {
                    tracing::info!(
                        "Post {} not updated since last fetch at {}, skipping",
                        post.id,
                        last_updated
                    );
                }
                continue;
            }

            let id = post.id;
            let mut tries = 0;
            let mut backoff = self.retry_backoff;
            let detail = loop {
                match crate::data::fanbox::fetch_post(session, id).await {
                    Err(e) => {
                        tracing::warn!("Failed to fetch post {}: {}", id, e);
                        if tries == self.retries {
                            tracing::error!("Failed to fetch post {} after {} tries", id, 1 + self.retries);

                            if !self.skip_failed {
                                return Err(e);
                            } else {
                                continue 'post;
                            }
                        }
                        tries += 1;
                        if let Some(ref mut b) = backoff {
                            tracing::info!("Backing off for {}ms", *b);
                            tokio::time::sleep(std::time::Duration::from_millis(*b as u64)).await;
                            *b *= 2;
                        }
                    }
                    Ok(d) => break d,
                }
            };

            let updated = crate::db::update_fanbox_post(&detail).await?;
            let prompt = match updated {
                crate::db::FanboxPostUpdateResult::Inserted => "Inserted",
                crate::db::FanboxPostUpdateResult::Updated => "Updated",
                crate::db::FanboxPostUpdateResult::Skipped => "Skipped",
            };

            tracing::info!("{} post {} - {}", prompt, id, detail.post.title);

            if let Some(ref body) = detail.body {
                for (idx, file) in body.files() {
                    let added = crate::db::add_fanbox_file(detail.post.id, idx, file).await?;
                    if added {
                        tracing::info!("  Added {}: file {} - {}", idx, file.id, file.name);
                    }
                }

                for (idx, image) in body.images() {
                    let added = crate::db::add_fanbox_image(detail.post.id, idx, image).await?;
                    if added {
                        tracing::info!("  Added {}: image {}", idx, image.id);
                    }
                }
            }

            if matches!(self.termination, TerminationCondition::OnHit)
                && matches!(updated, crate::db::FanboxPostUpdateResult::Updated)
            {
                tracing::info!("Encountered an already existing post. Terminating.");
                break;
            }
        }
        Ok(())
    }

    async fn sync_all(
        &self,
        session: &crate::config::Session,
    ) -> anyhow::Result<()> {
        let creators = crate::data::fanbox::fetch_supporting_list(session).await?;
        for creator in creators {
            tracing::info!(
                "Syncing creator {} ({})",
                creator
                    .user
                    .as_ref()
                    .map(|e| e.name.as_str())
                    .unwrap_or("?"),
                creator.creator_id
            );
            self.sync(session, &creator.creator_id).await?;
        }
        Ok(())
    }

    pub async fn run(&self, session: &crate::config::Session) -> anyhow::Result<()> {
        if let Some(ref c) = self.creator {
            self.sync(session, c).await
        } else {
            self.sync_all(session).await
        }
    }
}

#[derive(Subcommand)]
pub enum FanboxCmd {
    /// Synchronize posts from a Fanbox creator
    Sync(FanboxSyncArgs),

    /// Download a specific synced file or image
    Download {
        /// Type of the downloaded item
        #[arg(value_enum)]
        r#type: FanboxDownloadType,

        /// ID of the image / file
        id: String,

        /// Base directory to save the files
        ///
        /// The illustrations will be saved as `<base_dir>/<post_id>_<idx>_<image_id>[_<name>].<ext>`
        #[arg(short, long, default_value = "fanbox")]
        base_dir: String,

        /// Create base directory if not exist
        #[arg(long)]
        mkdir: bool,

        /// Canonicalization for paths recorded in database
        #[arg(long, value_enum, default_value_t = DatabasePathFormat::Absolute)]
        database_path_format: DatabasePathFormat,

        /// Show progress bar. The download speed is based on the *UNZIPPED* stream, so don't be surprised if it exceeds your bandwidth.
        #[arg(short, long)]
        progress: bool,
    },
}

impl Fanbox {
    pub async fn run(self, session: &crate::config::Session) -> anyhow::Result<()> {
        match self.cmd {
            FanboxCmd::Sync(sync) => sync.run(session).await?,
            FanboxCmd::Download {
                r#type,
                id,
                database_path_format,
                base_dir,
                mkdir,
                progress,
            } => {
                if mkdir {
                    std::fs::create_dir_all(&base_dir)?;
                }
                let (url, filename) = get_download_spec(r#type, &id).await?;
                let DownloadResult { written_path, .. } = crate::util::download_then_persist(
                    FanboxRequest(session),
                    &base_dir,
                    &filename,
                    database_path_format,
                    &url,
                    progress,
                )
                .await?;
                let updated = match r#type {
                    FanboxDownloadType::File => {
                        crate::db::update_file_download(&id, written_path.to_str().unwrap()).await?
                    }
                    FanboxDownloadType::Image => {
                        crate::db::update_image_download(&id, written_path.to_str().unwrap())
                            .await?
                    }
                };

                assert!(
                    updated,
                    "{} {} should exist in database. Possible DB race",
                    match r#type {
                        FanboxDownloadType::File => "File",
                        FanboxDownloadType::Image => "Image",
                    },
                    id
                );
            }
        }
        Ok(())
    }
}

/// Return (url, filename)
async fn get_download_spec(ty: FanboxDownloadType, id: &str) -> anyhow::Result<(String, String)> {
    match ty {
        FanboxDownloadType::File => {
            let spec = crate::db::query_fanbox_file_dwn(id)
                .await?
                .ok_or_else(|| anyhow::anyhow!("File {} not found in database", id))?;
            let filename = format!(
                "{}_{}_{}_{}.{}",
                spec.post_id, spec.idx, id, spec.name, spec.ext
            );
            Ok((spec.url, filename))
        }
        FanboxDownloadType::Image => {
            let spec = crate::db::query_fanbox_image_dwn(id)
                .await?
                .ok_or_else(|| anyhow::anyhow!("Image {} not found in database", id))?;
            let filename = format!("{}_{}_{}.{}", spec.post_id, spec.idx, id, spec.ext);
            Ok((spec.url, filename))
        }
    }
}
