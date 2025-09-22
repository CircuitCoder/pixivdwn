mod cmd;
mod config;
mod data;
mod db;
mod image;

use clap::Parser;
#[derive(Parser)]
struct Args {
    #[arg(long)]
    /// Pixiv Cookie, not including the "PHPSESSID=" prefix
    /// Can also be set via the PIXIV_COOKIE environment variable
    pixiv_cookie: Option<String>,

    #[command(subcommand)]
    command: cmd::Command,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    dotenvy::dotenv()?;
    tracing_subscriber::fmt::init();
    let args = Args::parse();

    let pixiv_cookie = args
        .pixiv_cookie
        .or_else(|| std::env::var("PIXIV_COOKIE").ok());
    let session = config::Session::new(pixiv_cookie, None)?;
    args.command.run(&session).await?;

    Ok(())
}
