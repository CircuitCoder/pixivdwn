mod data;
mod config;
mod db;

use std::collections::HashMap;

use clap::Parser;

#[derive(Parser)]
struct Args {
    #[arg(long)]
    /// Pixiv Cookie, quotient of PHPSESSID
    pixiv_cookie: String,

    #[arg(short, long)]
    /// Bookmark tag
    tag: Option<String>,

    #[arg(short, long)]
    /// Private or not
    private: bool,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    dotenvy::dotenv()?;
    tracing_subscriber::fmt::init();
    let args = Args::parse();

    let session = config::Session::new(args.pixiv_cookie, None)?;
    let bookmarks = data::get_bookmarks(&session, args.tag.as_deref(), args.private).await?;
    let mut tag_map_ctx: HashMap<&str, u64> = HashMap::new();
    for illust in bookmarks.values() {
        let updated = db::update_illust(illust, &mut tag_map_ctx).await?;
        println!("{}: {}{}", illust.id, if updated { "[UPDATED] " } else { "" }, illust.data.display_title());
    }

    Ok(())
}
