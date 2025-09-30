use std::collections::HashMap;

use clap::Args;

#[derive(Args)]
pub struct Illust {
    /// Illustration ID
    id: u64,

    /// Dry run, only fech and print the info
    #[arg(long)]
    dry_run: bool,
}

impl Illust {
    pub async fn run(self, session: &crate::config::Session) -> anyhow::Result<()> {
        let illust = crate::data::pixiv::get_illust(session, self.id).await?;
        if self.dry_run {
            tracing::info!("Fetched: {:?}", illust);
            return Ok(());
        }

        let mut tag_map_ctx: HashMap<String, u64> = HashMap::new();
        crate::db::update_illust(&illust, &mut tag_map_ctx).await?;
        Ok(())
    }
}
