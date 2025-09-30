use clap::Args;
use futures::StreamExt;

#[derive(Args)]
pub struct Fanbox {
    /// ID of the Fanbox creator
    creator: String,
}

impl Fanbox {
    pub async fn run(self, session: &crate::config::Session) -> anyhow::Result<()> {
        let mut posts = Box::pin(crate::data::fanbox::fetch_author_posts(session, &self.creator));
        while let Some(post) = posts.next().await.transpose()? {
            tracing::info!("{:#?}", post);
        }
        Ok(())
    }
}
