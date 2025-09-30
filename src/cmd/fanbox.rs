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
            let delay = std::time::Duration::from_millis((DELAY_MS + rand::random_range(-DELAY_RANDOM_VAR_MS..=DELAY_RANDOM_VAR_MS)) as u64);
            let id = post.id;
            tokio::time::sleep(delay).await;

            let detail = crate::data::fanbox::fetch_post(session, id).await?;
            println!("Post ID: {}, blocks: {:?}", detail.post.id, detail.body.blocks);
        }
        Ok(())
    }
}
