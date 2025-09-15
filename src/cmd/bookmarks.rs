use std::collections::HashMap;

use clap::Args;
use futures::{StreamExt, pin_mut};

#[derive(clap::ValueEnum, Clone, Copy, PartialEq, Eq)]
enum TerminationCondition {
    /// Terminate when an already existing illustration is encountered
    OnHit,

    /// Terminate until no more illustrations are available
    UntilEnd,
}

#[derive(Args)]
pub struct Bookmarks {
    #[arg(short, long)]
    /// Bookmark tag
    tag: Option<String>,

    #[arg(long, default_value = "0")]
    /// Initial offset
    offset: usize,

    #[arg(long)]
    /// Maximum number of fetched illustrations
    max_cnt: Option<usize>,

    #[arg(short, long)]
    /// Fetch private bookmarks
    private: bool,

    #[arg(alias="term", long, value_enum, default_value_t = TerminationCondition::UntilEnd)]
    /// Termination condition (alias: --term)
    termination: TerminationCondition,
}

impl Bookmarks {
    pub async fn run(self, session: &crate::config::Session) -> anyhow::Result<()> {
        let bookmarks =
            crate::data::get_bookmarks(&session, self.tag.as_deref(), self.offset, self.private)
                .await;
        pin_mut!(bookmarks);
        let mut tag_map_ctx: HashMap<String, u64> = HashMap::new();
        let mut cnt = 0;
        while let Some(illust) = bookmarks.next().await {
            let illust = illust?;
            let updated = crate::db::update_illust(&illust, &mut tag_map_ctx).await?;
            tracing::info!(
                "Queried {}: {}{}",
                illust.id,
                if updated { "[UPDATED] " } else { "" },
                illust.data.display_title()
            );

            if !updated && self.termination == TerminationCondition::OnHit {
                tracing::info!("Encountered an already existing illustration. Terminating.");
                break;
            }

            cnt += 1;
            if let Some(max_count) = self.max_cnt
                && cnt >= max_count
            {
                tracing::info!(
                    "Reached the maximum number of fetched illustrations ({}). Terminating.",
                    max_count
                );
                break;
            }
        }
        Ok(())
    }
}
