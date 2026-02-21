use std::collections::HashMap;

use clap::Args;
use futures::{StreamExt, pin_mut};

use crate::util::TerminationCondition;

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
    pub async fn run(
        self,
        session: &crate::config::Session,
        db: &crate::db::Database,
    ) -> anyhow::Result<()> {
        let bookmarks = crate::data::pixiv::get_bookmarks(
            &session,
            self.tag.as_deref(),
            self.offset,
            self.private,
        )
        .await;
        pin_mut!(bookmarks);
        let mut tag_map_ctx: HashMap<String, u64> = HashMap::new();
        let mut cnt = 0;
        while let Some(illust) = bookmarks.next().await {
            let illust = illust?;
            let update_result = db.update_illust(&illust, &mut tag_map_ctx).await?;
            let update_prompt = match update_result {
                crate::db::IllustUpdateResult::Inserted => "INSERTED",
                crate::db::IllustUpdateResult::BookmarkIDChanged => "BMIDCHANGED",
                crate::db::IllustUpdateResult::Updated => "UPDATED",
                crate::db::IllustUpdateResult::Skipped => "SKIPPED",
            };
            tracing::info!(
                "Queried {}: [{}] {}",
                illust.id,
                update_prompt,
                illust.data.display_title()
            );

            if update_result == crate::db::IllustUpdateResult::Updated
                && self.termination == TerminationCondition::OnHit
            {
                tracing::info!(
                    "Encountered an already existing illustration, whose bookmark ID is unchanged. Terminating."
                );
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
