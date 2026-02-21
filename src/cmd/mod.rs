pub mod bookmarks;
pub mod database;
pub mod download;
pub mod fanbox;
pub mod illust;
pub mod query;

use clap::Subcommand;

#[derive(Subcommand)]
pub enum Command {
    /// Sync bookmarks into database
    Bookmarks(bookmarks::Bookmarks),

    /// Sync individual illustration by ID
    Illust(illust::Illust),

    /// Download individual illustration by ID
    Download(download::Download),

    /// Fanbox fetching
    Fanbox(fanbox::Fanbox),

    /// Query local database
    Query(query::Query),

    /// Database management
    Database(database::Database),
}

impl Command {
    pub async fn run(self, session: &crate::config::Session, dburl: &str) -> anyhow::Result<()> {
        if let Command::Database(cmd) = self {
            return cmd.run(session, dburl).await;
        }

        let db = crate::db::Database::load(dburl).await?;

        match self {
            Command::Bookmarks(cmd) => cmd.run(session, &db).await,
            Command::Illust(cmd) => cmd.run(session, &db).await,
            Command::Download(cmd) => cmd.run(session, &db).await,
            Command::Fanbox(cmd) => cmd.run(session, &db).await,
            Command::Query(cmd) => cmd.run(&db).await,
            Command::Database(_) => unreachable!(),
        }
    }
}
