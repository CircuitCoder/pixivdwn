use serde::de::DeserializeOwned;

// Rate-limiter
static CTX: tokio::sync::Mutex<Option<(
    wreq::Client,
    tokio::time::Instant
)>> = tokio::sync::Mutex::const_new(None);

const DELAY_MS: i64 = 2500;
const DELAY_RANDOM_VAR_MS: i64 = 500;

#[inline]
pub async fn fetch<T: DeserializeOwned>(req: impl FnOnce(&wreq::Client) -> anyhow::Result<wreq::Request>) -> anyhow::Result<T> {
    let mut next = CTX.lock().await;
    let (client_ref, ddl_ref) = match &mut *next {
        None => {
            let client = wreq::Client::new();
            *next = Some((client, tokio::time::Instant::now()));
            let tuple = next.as_mut().unwrap();
            (&mut tuple.0, &mut tuple.1)
        },
        Some((client, ddl)) => {
            tokio::time::sleep_until(*ddl).await;
            (client, ddl)
        }
    };


    let req = req(client_ref)?;
    async fn fetch_inner<T: DeserializeOwned>(client_ref: &wreq::Client, req: wreq::Request) -> anyhow::Result<T> {
        let resp = client_ref.execute(req).await?;
        let json = resp.json::<T>().await?;
        Ok(json)
    }

    let ret = fetch_inner(client_ref, req).await;
    let delay = std::time::Duration::from_millis(
        (DELAY_MS + rand::random_range(-DELAY_RANDOM_VAR_MS..=DELAY_RANDOM_VAR_MS)) as u64,
    );
    *ddl_ref = tokio::time::Instant::now() + delay;

    ret
}
