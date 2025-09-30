use clap::Args;
use futures::StreamExt;

#[derive(Args)]
pub struct Fanbox {
    /// ID of the Fanbox creator
    creator: String,
}

impl Fanbox {
    pub async fn run(self, session: &crate::config::Session) -> anyhow::Result<()> {
        const DELAY_MS: i64 = 2500;
        const DELAY_RANDOM_VAR_MS: i64 = 500;

        let mut posts = Box::pin(crate::data::fanbox::fetch_author_posts(session, &self.creator));
        while let Some(post) = posts.next().await.transpose()? {
            let last_updated = crate::db::query_fanbox_post_updated_datetime(post.id).await?;
            if let Some(last_updated) = last_updated && last_updated >= post.updated_datetime {
                if last_updated > post.updated_datetime {
                    tracing::warn!("Post {} updated_datetime went backwards: was {}, now {}", post.id, last_updated, post.updated_datetime);
                } else {
                    tracing::info!("Post {} not updated since last fetch at {}, skipping remaining posts", post.id, last_updated);
                }
                continue;
            }

            let delay = std::time::Duration::from_millis((DELAY_MS + rand::random_range(-DELAY_RANDOM_VAR_MS..=DELAY_RANDOM_VAR_MS)) as u64);
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

            for file in detail.body.files() {
                let added = crate::db::add_fanbox_file(detail.post.id, file).await?;
                if added {
                    tracing::info!("| Added file {} - {}", file.id, file.name);
                }
            }

            for image in detail.body.images() {
                let added = crate::db::add_fanbox_image(detail.post.id, image).await?;
                if added {
                    tracing::info!("| Added image {}", image.id);
                }
            }
        }
        Ok(())
    }
}
