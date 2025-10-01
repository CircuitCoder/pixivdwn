use clap::{Args, Subcommand};
use futures::StreamExt;

use crate::data::fanbox::FanboxRequest;

#[derive(Args)]
pub struct Fanbox {
    #[command(subcommand)]
    cmd: FanboxCmd,

    /// Base directory to save the files
    ///
    /// The illustrations will be saved as `<base_dir>/<post_id>_<idx>_<image_id>[_<name>].<ext>`
    #[arg(short, long, default_value = "fanbox")]
    base_dir: String,
}

#[derive(Subcommand)]
pub enum FanboxCmd {
    /// Synchronize posts from a Fanbox creator
    SyncCreator {
        /// ID of the Fanbox creator
        creator: String,
    },

    DownloadFile {
        /// ID of the Fanbox file
        id: String,
    },

    DownloadImage {
        /// ID of the Fanbox image
        id: String,
    },
}

impl Fanbox {
    pub async fn run(self, session: &crate::config::Session) -> anyhow::Result<()> {
        match self.cmd {
            FanboxCmd::SyncCreator { creator } => {
                sync_creator(session, &creator).await?;
            }
            FanboxCmd::DownloadFile { id } => {
                let spec = crate::db::query_fanbox_file_dwn(&id)
                    .await?
                    .ok_or_else(|| anyhow::anyhow!("File {} not found in database", id))?;
                let filename = format!(
                    "{}_{}_{}_{}.{}",
                    spec.post_id, spec.idx, id, spec.name, spec.ext
                );
                // FIXME: Let's get a quick hack
                let final_path = std::path::Path::new(&self.base_dir).join(&filename);
                tracing::info!("Downloading file {} to {}", id, final_path.display());
                if final_path.exists() {
                    return Err(anyhow::anyhow!(
                        "File already exists: {}",
                        final_path.display()
                    ));
                }

                crate::data::file::download(
                    FanboxRequest(session),
                    &spec.url,
                    std::io::BufWriter::new(std::fs::File::create(&final_path)?),
                    true,
                )
                .await?;

                let updated =
                    crate::db::update_file_download(&id, final_path.to_str().unwrap()).await?;
                assert!(updated, "File {} should exist in database", id);
            }
            FanboxCmd::DownloadImage { id } => {
                let spec = crate::db::query_fanbox_image_dwn(&id)
                    .await?
                    .ok_or_else(|| anyhow::anyhow!("Image {} not found in database", id))?;
                let filename = format!("{}_{}_{}.{}", spec.post_id, spec.idx, id, spec.ext);

                let final_path = std::path::Path::new(&self.base_dir).join(&filename);
                tracing::info!("Downloading image {} to {}", id, final_path.display());
                if final_path.exists() {
                    return Err(anyhow::anyhow!(
                        "File already exists: {}",
                        final_path.display()
                    ));
                }

                crate::data::file::download(
                    FanboxRequest(session),
                    &spec.url,
                    std::io::BufWriter::new(std::fs::File::create(&final_path)?),
                    true,
                )
                .await?;

                let updated =
                    crate::db::update_image_download(&id, final_path.to_str().unwrap()).await?;
                assert!(updated, "Image {} should exist in database", id);
            }
        }
        Ok(())
    }
}

async fn sync_creator(session: &crate::config::Session, creator: &str) -> anyhow::Result<()> {
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
