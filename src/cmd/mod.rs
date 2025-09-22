pub mod bookmarks;
pub mod illust;

use clap::Subcommand;

#[derive(Subcommand)]
pub enum Command {
    /// Sync bookmarks into database
    Bookmarks(bookmarks::Bookmarks),

    /// Download / sync individual illustration by ID
    Illust(illust::Illust),
}

impl Command {
    pub async fn run(self, session: &crate::config::Session) -> anyhow::Result<()> {
        match self {
            Command::Bookmarks(cmd) => cmd.run(session).await,
            Command::Illust(cmd) => cmd.run(session).await,
        }
    }
}
