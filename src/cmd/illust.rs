use std::collections::HashMap;

use clap::Args;

#[derive(clap::ValueEnum, Clone, Copy, PartialEq, Eq)]
enum FetchMode {
    /// Only fetch metadata such as title, descriptions, etc.
    MetadataOnly,

    /// Download illustrations as well
    Full,
}

#[derive(Args)]
pub struct Illust {
    /// Illustration ID
    id: u64,

    /// Fetch mode
    #[arg(short, long, value_enum, default_value_t = FetchMode::Full)]
    mode: FetchMode,

    /// Dry run, only fech and print the info
    #[arg(long)]
    dry_run: bool,
}

impl Illust {
    pub async fn run(self, session: &crate::config::Session) -> anyhow::Result<()> {
        let illust = crate::data::get_illust(session, self.id).await?;
        if self.dry_run {
            tracing::info!("Fetched: {:?}", illust);
            return Ok(());
        }

        let mut tag_map_ctx: HashMap<String, u64> = HashMap::new();
        crate::db::update_illust(&illust, &mut tag_map_ctx).await?;
        Ok(())
    }
}
