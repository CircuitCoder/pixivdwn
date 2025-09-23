use clap::{Args, Subcommand};

#[derive(Args)]
pub struct Database {
    #[command(subcommand)]
    cmd: DatabaseCmd,
}

#[derive(Subcommand)]
pub enum DatabaseCmd {
    /// Setup / migrate the database
    Setup,
}

impl Database {
    pub async fn run(self) -> anyhow::Result<()> {
        match self.cmd {
            DatabaseCmd::Setup => self.setup().await,
        }
    }

    pub async fn setup(self) -> anyhow::Result<()> {
        crate::db::setup_db().await?;
        Ok(())
    }
}
