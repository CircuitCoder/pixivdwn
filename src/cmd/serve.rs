use clap::Args;
use std::net::IpAddr;

#[derive(Args)]
pub struct Serve {
    #[arg(long, default_value = "6103")]
    port: u16,

    #[arg(long, default_value = "0.0.0.0")]
    host: IpAddr,
}

impl Serve {
    pub async fn run(&self, db: crate::db::Database) -> anyhow::Result<()> {
        let app = crate::serve::build(db);
        let binder = tokio::net::TcpListener::bind((self.host, self.port)).await?;
        tracing::info!("Listening on {}:{}", self.host, self.port);
        axum::serve(binder, app).await?;

        Ok(())
    }
}
