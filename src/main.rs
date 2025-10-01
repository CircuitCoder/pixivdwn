mod cmd;
mod config;
mod data;
mod db;
mod fetch;
mod util;

use clap::Parser;
#[derive(Parser)]
struct Args {
    #[arg(long)]
    /// Pixiv Cookie, not including the "PHPSESSID=" prefix
    /// Can also be set via the PIXIV_COOKIE environment variable
    pixiv_cookie: Option<String>,

    /// Fanbox cookie, not including the "FANBOXSESSID=" prefix
    /// Can also be set via the FANBOX_COOKIE environment variable
    fanbox_cookie: Option<String>,

    /// Full Fanbox cookie, including other fields such as Cloudflare tokens
    /// Overrides `fanbox_cookie` if both are set
    fanbox_cookie_full: Option<String>,

    /// Database URL, Can also be set via the DATABASE_URL environment variable
    #[arg(long)]
    database_url: Option<String>,

    /// Override fetch delay (ms)
    #[arg(long, default_value_t = 2500)]
    fetch_delay: i64,

    /// Override fetch delay random variance (ms)
    #[arg(long, default_value_t = 500)]
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
    crate::db::set_url(database_url).await?;

    let pixiv_cookie = args
        .pixiv_cookie
        .or_else(|| std::env::var("PIXIV_COOKIE").ok());

    let fanbox_cookie = args
        .fanbox_cookie
        .or_else(|| std::env::var("FANBOX_COOKIE").ok());

    let fanbox_cookie_full = args
        .fanbox_cookie_full
        .or_else(|| std::env::var("FANBOX_COOKIE_FULL").ok());

    let session = config::Session::new(pixiv_cookie, fanbox_cookie, fanbox_cookie_full)?;
    args.command.run(&session).await?;

    Ok(())
}
