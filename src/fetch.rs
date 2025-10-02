use std::sync::atomic::AtomicI64;

use serde::de::DeserializeOwned;

// Rate-limiter
type Ctx = (wreq::Client, tokio::time::Instant);
static CTX: tokio::sync::Mutex<Option<Ctx>> = tokio::sync::Mutex::const_new(None);

static DELAY_MS: AtomicI64 = AtomicI64::new(2500);
static DELAY_RANDOM_VAR_MS: AtomicI64 = AtomicI64::new(500);

pub fn update_delay_settings(base: i64, var: i64) {
    DELAY_MS.store(base, std::sync::atomic::Ordering::Relaxed);
    DELAY_RANDOM_VAR_MS.store(var, std::sync::atomic::Ordering::Relaxed);
}

pub struct FetchCtxGuard<'a> {
    guard: tokio::sync::MutexGuard<'a, Option<Ctx>>,
}

impl<'a> FetchCtxGuard<'a> {
    pub async fn begin() -> FetchCtxGuard<'static> {
        let mut next = CTX.lock().await;
        match &mut *next {
            None => {
                let client = wreq::Client::new();
                *next = Some((client, tokio::time::Instant::now()));
            }
            Some((_, ddl)) => {
                tokio::time::sleep_until(*ddl).await;
            }
        };

        FetchCtxGuard { guard: next }
    }

    pub fn client(&self) -> &wreq::Client {
        &self.guard.as_ref().unwrap().0
    }
}

impl Drop for FetchCtxGuard<'_> {
    fn drop(&mut self) {
        let var = DELAY_RANDOM_VAR_MS.load(std::sync::atomic::Ordering::Relaxed);
        let base = DELAY_MS.load(std::sync::atomic::Ordering::Relaxed);
        let delay =
            std::time::Duration::from_millis((base + rand::random_range(-var..=var)) as u64);
        self.guard.as_mut().unwrap().1 = tokio::time::Instant::now() + delay;
    }
}

#[inline]
pub async fn fetch<T: DeserializeOwned>(
    req: impl FnOnce(&wreq::Client) -> anyhow::Result<wreq::Request>,
) -> anyhow::Result<T> {
    let ctx = FetchCtxGuard::begin().await;

    let client = ctx.client();
    let req = req(client)?;
    tracing::debug!("Fetching {}", req.uri());
    tracing::debug!("  Headers: {:#?}", req.headers());
    let resp = client.execute(req).await?;
    let json = resp.json::<T>().await?;
    Ok(json)
}
