use clap::{Args, Subcommand};
use futures::StreamExt;

use crate::{
    data::fanbox::FanboxRequest,
    util::{DatabasePathFormat, DownloadResult},
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

#[derive(Subcommand)]
pub enum FanboxCmd {
    /// Synchronize posts from a Fanbox creator
    Sync {
        /// ID of the Fanbox creator
        creator: String,
    },

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
            FanboxCmd::Sync { creator } => {
                sync(session, &creator).await?;
            }
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

async fn sync(session: &crate::config::Session, creator: &str) -> anyhow::Result<()> {
    const DELAY_MS: i64 = 2500;
    const DELAY_RANDOM_VAR_MS: i64 = 500;

    let mut posts = Box::pin(crate::data::fanbox::fetch_author_posts(session, creator));
    while let Some(post) = posts.next().await.transpose()? {
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

        let delay = std::time::Duration::from_millis(
            (DELAY_MS + rand::random_range(-DELAY_RANDOM_VAR_MS..=DELAY_RANDOM_VAR_MS)) as u64,
        );
        let id = post.id;
        tokio::time::sleep(delay).await;

        let detail = crate::data::fanbox::fetch_post(session, id).await?;

        let updated = crate::db::update_fanbox_post(&detail).await?;
        let prompt = match updated {
            crate::db::FanboxPostUpdateResult::Inserted => "Inserted",
            crate::db::FanboxPostUpdateResult::Updated => "Updated",
            crate::db::FanboxPostUpdateResult::Skipped => "Skipped",
        };

        tracing::info!("{} post {} - {}", prompt, id, detail.post.title);

        for (idx, file) in detail.body.files() {
            let added = crate::db::add_fanbox_file(detail.post.id, idx, file).await?;
            if added {
                tracing::info!("  Added {}: file {} - {}", idx, file.id, file.name);
            }
        }

        for (idx, image) in detail.body.images() {
            let added = crate::db::add_fanbox_image(detail.post.id, idx, image).await?;
            if added {
                tracing::info!("  Added {}: image {}", idx, image.id);
            }
        }
    }
    Ok(())
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
