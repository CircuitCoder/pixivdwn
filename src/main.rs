mod data;
mod config;
mod db;

use std::collections::HashMap;
use futures::{pin_mut, stream::StreamExt};

use clap::Parser;

#[derive(Parser)]
struct Args {
    #[arg(long)]
    /// Pixiv Cookie, quotient of PHPSESSID
    pixiv_cookie: String,

    #[arg(short, long)]
    /// Bookmark tag
    tag: Option<String>,

    #[arg(long, default_value = "0")]
    /// Initial offset
    offset: usize,

    #[arg(short, long)]
    /// Fetch private bookmarks
    private: bool,

    #[arg(alias="term", long, value_enum, default_value_t = TerminationCondition::UntilEnd)]
    /// Termination condition (alias: --term)
    termination: TerminationCondition,
}

#[derive(clap::ValueEnum, Clone, Copy, PartialEq, Eq)]
enum TerminationCondition {
    /// Terminate when an already existing illustration is encountered
    OnHit,

    /// Terminate until no more illustrations are available
    UntilEnd,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    dotenvy::dotenv()?;
    tracing_subscriber::fmt::init();
    let args = Args::parse();

    let session = config::Session::new(args.pixiv_cookie, None)?;
    let bookmarks = data::get_bookmarks(&session, args.tag.as_deref(), args.offset, args.private).await;
    pin_mut!(bookmarks);
    let mut tag_map_ctx: HashMap<String, u64> = HashMap::new();
    while let Some(illust) = bookmarks.next().await {
        let illust = illust?;
        let updated = db::update_illust(&illust, &mut tag_map_ctx).await?;
        tracing::info!("Queried {}: {}{}", illust.id, if updated { "[UPDATED] " } else { "" }, illust.data.display_title());
        if !updated && args.termination == TerminationCondition::OnHit {
            tracing::info!("Encountered an already existing illustration. Terminating.");
            break;
        }
    }

    Ok(())
}
