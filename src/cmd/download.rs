use clap::Args;

#[derive(Args)]
pub struct Download {
    /// Illustration ID
    id: u64,

    /// Dry run, only fech and print the info
    #[arg(long)]
    dry_run: bool,
}

impl Download {
    pub async fn run(self, session: &crate::config::Session) -> anyhow::Result<()> {
        let illust = crate::data::get_illust_pages(session, self.id).await?;
        if self.dry_run {
            tracing::info!("Fetched: {:?}", illust);
            return Ok(());
        }

        Ok(())
    }
}
