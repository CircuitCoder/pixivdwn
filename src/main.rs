mod cmd;
mod config;
mod data;
mod db;
mod fetch;
mod util;

use std::path::PathBuf;

use clap::Parser;
#[derive(Parser)]
#[command(version, about)]
struct Args {
    #[arg(long)]
    /// Pixiv Cookie, not including the "PHPSESSID=" prefix
    /// Can also be set via the PIXIV_COOKIE environment variable
    #[arg(long, hide_short_help = true)]
    pixiv_cookie: Option<String>,

    /// Fanbox cookie, not including the "FANBOXSESSID=" prefix
    /// Can also be set via the FANBOX_COOKIE environment variable
    #[arg(long, hide_short_help = true)]
    fanbox_cookie: Option<String>,

    /// Full Fanbox headers, including other ones such as UA and full cookie
    /// Can also be set via the FANBOX_HEADER_FULL environment variable
    /// Overrides `fanbox_cookie` if both are set
    #[arg(long, hide_short_help = true)]
    fanbox_header_full: Option<String>,

    /// Base directory to save / lookup pixiv illustrations
    ///
    /// The illustrations will be saved as `<base_dir>/<illust_id>_p<page>.<ext>`
    /// Can also be set via the PIXIV_BASE_DIR environment variable
    #[arg(long, hide_short_help = true)]
    pixiv_base_dir: Option<PathBuf>,

    /// Base directory to save / lookup fanbox illustrations
    ///
    /// The illustrations will be saved as `<base_dir>/<post_id>_<idx>_<image_id>[_<name>].<ext>`
    /// Can also be set via the FANBOX_BASE_DIR environment variable
    #[arg(long, hide_short_help = true)]
    fanbox_base_dir: Option<PathBuf>,

    /// Database URL, Can also be set via the DATABASE_URL environment variable
    #[arg(long, hide_short_help = true)]
    database_url: Option<String>,

    /// Override fetch delay (ms)
    #[arg(long, default_value_t = 2500, hide_short_help = true)]
    fetch_delay: i64,

    /// Override fetch delay random variance (ms)
    #[arg(long, default_value_t = 500, hide_short_help = true)]
    fetch_delay_var: i64,

    #[command(subcommand)]
    command: cmd::Command,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    dotenvy::dotenv()?;
    tracing_subscriber::fmt::init();
    let args = Args::parse();

    fetch::update_delay_settings(args.fetch_delay, args.fetch_delay_var);

    let database_url = args.database_url.or_else(|| std::env::var("DATABASE_URL").ok())
        .ok_or_else(|| anyhow::anyhow!("Please specify a database URL via --database-url or the DATABASE_URL environment variable"))?;

    let pixiv_cookie = args
        .pixiv_cookie
        .or_else(|| std::env::var("PIXIV_COOKIE").ok());

    let fanbox_cookie = args
        .fanbox_cookie
        .or_else(|| std::env::var("FANBOX_COOKIE").ok());

    let fanbox_header_full = args
        .fanbox_header_full
        .or_else(|| std::env::var("FANBOX_HEADER_FULL").ok());

    let pixiv_base_dir = args
        .pixiv_base_dir
        .or_else(|| std::env::var("PIXIV_BASE_DIR").ok().map(PathBuf::from));
    let fanbox_base_dir = args
        .fanbox_base_dir
        .or_else(|| std::env::var("FANBOX_BASE_DIR").ok().map(PathBuf::from));

    let session = config::Session::new(
        pixiv_cookie,
        fanbox_cookie,
        fanbox_header_full,
        pixiv_base_dir,
        fanbox_base_dir,
    )?;
    args.command.run(&session, &database_url).await?;

    Ok(())
}
