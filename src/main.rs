mod data;
mod config;

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
    tracing_subscriber::fmt::init();
    let args = Args::parse();

    let session = config::Session::new(args.pixiv_cookie, None)?;
    let bookmarks = data::get_bookmarks(&session, args.tag.as_deref(), args.private).await?;
    for (_, illust) in bookmarks {
        println!("{}: {}", illust.id, illust.data.display_title());
    }

    Ok(())
}
