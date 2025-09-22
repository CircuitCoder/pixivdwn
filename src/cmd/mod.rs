pub mod bookmarks;
pub mod illust;
pub mod download;

use clap::Subcommand;

#[derive(Subcommand)]
pub enum Command {
    /// Sync bookmarks into database
    Bookmarks(bookmarks::Bookmarks),

    /// Sync individual illustration by ID
    Illust(illust::Illust),

    /// Download individual illustration by ID
    Download(download::Download),
}

impl Command {
    pub async fn run(self, session: &crate::config::Session) -> anyhow::Result<()> {
        match self {
            Command::Bookmarks(cmd) => cmd.run(session).await,
            Command::Illust(cmd) => cmd.run(session).await,
            Command::Download(cmd) => cmd.run(session).await,

        }
    }
}
