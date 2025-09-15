pub mod bookmarks;

use clap::Subcommand;

#[derive(Subcommand)]
pub enum Command {
    // Sync bookmarks into database
    Bookmarks(bookmarks::Bookmarks),
}

impl Command {
    pub async fn run(self, session: &crate::config::Session) -> anyhow::Result<()> {
        match self {
            Command::Bookmarks(cmd) => cmd.run(session).await,
        }
    }
}
