mod data;
mod config;
mod db;

use std::collections::{HashMap, HashSet};

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
    let mut tag_map: HashMap<&str, u64> = HashMap::new();
    for b in bookmarks.values() {
        use std::collections::hash_map::Entry::*;
        for t in &b.bookmark.tags {
            let e = tag_map.entry(t);
            if let Vacant(v) = e {
                let id = db::get_tag_mapping(t).await?;
                v.insert(id);
            }
        }
        if let data::IllustData::Fetched { tags, .. } = &b.data {
            for t in tags {
                let e = tag_map.entry(t);
                if let Vacant(v) = e {
                    let id = db::get_tag_mapping(t).await?;
                    v.insert(id);
                }
            }
        }
    }

    for (_, illust) in bookmarks {
        println!("{}: {}", illust.id, illust.data.display_title());
    }

    Ok(())
}
