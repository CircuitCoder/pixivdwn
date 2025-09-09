mod data;
mod config;

use clap::Parser;

#[derive(Parser)]
struct Args {
    #[arg(short, long)]
    token: String,
}

fn main() -> anyhow::Result<()> {
    let args = Args::parse();
    Ok(())
}
