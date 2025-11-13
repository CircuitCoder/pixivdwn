use std::collections::HashMap;

use clap::Args;

use crate::util::DownloadIdSrc;

#[derive(Args)]
pub struct Illust {
    #[clap(flatten)]
    /// ID of the illustration
    id: DownloadIdSrc<u64>,

    /// Dry run, only fech and print the info
    #[arg(long)]
    dry_run: bool,

    /// Abort if failed
    #[arg(long)]
    abort_on_fail: bool,
}

impl Illust {
    pub async fn run(self, session: &crate::config::Session) -> anyhow::Result<()> {
        let mut errored = 0;
        for id in self.id.read()? {
            let id = id?;
            let ret = self.sync_single(session, id).await;
            if ret.is_err() {
                if self.abort_on_fail {
                    return ret;
                } else {
                    tracing::error!("Failed to sync illust {}: {:?}", id, ret.err());
                    errored += 1;
                }
            } else {
            }
        }

        if errored == 0 {
            Ok(())
        } else {
            return Err(anyhow::anyhow!("{} illust(s) failed to sync", errored));
        }
    }

    async fn sync_single(&self, session: &crate::config::Session, id: u64) -> anyhow::Result<()> {
        let illust = crate::data::pixiv::get_illust(session, id).await?;

        if !self.dry_run {
            let mut tag_map_ctx: HashMap<String, u64> = HashMap::new();
            let update_result = crate::db::update_illust(&illust, &mut tag_map_ctx).await?;
            let update_prompt = match update_result {
                crate::db::IllustUpdateResult::Inserted => "INSERTED",
                crate::db::IllustUpdateResult::BookmarkIDChanged => "BMIDCHANGED",
                crate::db::IllustUpdateResult::Updated => "UPDATED",
                crate::db::IllustUpdateResult::Skipped => "SKIPPED",
            };
            tracing::info!(
                "Synced {}: [{}] {}",
                illust.id,
                update_prompt,
                illust.data.display_title()
            );
        } else {
            tracing::info!("Fetched {}: {}", illust.id, illust.data.display_title());
        }
        Ok(())
    }
}
