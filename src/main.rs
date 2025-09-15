mod data;
mod config;
mod db;
mod cmd;

use clap::Parser;
#[derive(Parser)]
struct Args {
    #[arg(long)]
    /// Pixiv Cookie, quotient of PHPSESSID
    pixiv_cookie: String,

    #[command(subcommand)]
    command: cmd::Command,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    dotenvy::dotenv()?;
    tracing_subscriber::fmt::init();
    let args = Args::parse();

    let session = config::Session::new(args.pixiv_cookie, None)?;
    args.command.run(&session).await?;

    Ok(())
}
