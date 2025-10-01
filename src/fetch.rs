use serde::de::DeserializeOwned;

// Rate-limiter
type Ctx = (wreq::Client, tokio::time::Instant);
static CTX: tokio::sync::Mutex<Option<Ctx>> = tokio::sync::Mutex::const_new(None);

const DELAY_MS: i64 = 2500;
const DELAY_RANDOM_VAR_MS: i64 = 500;

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
            },
            Some((_, ddl)) => {
                tokio::time::sleep_until(*ddl).await;
            }
        };

        FetchCtxGuard {
            guard: next,
        }
    }

    pub fn client(&self) -> &wreq::Client {
        &self.guard.as_ref().unwrap().0
    }
}

impl Drop for FetchCtxGuard<'_> {
    fn drop(&mut self) {
        let delay = std::time::Duration::from_millis(
            (DELAY_MS + rand::random_range(-DELAY_RANDOM_VAR_MS..=DELAY_RANDOM_VAR_MS)) as u64,
        );
        self.guard.as_mut().unwrap().1 = tokio::time::Instant::now() + delay;
    }
}

#[inline]
pub async fn fetch<T: DeserializeOwned>(req: impl FnOnce(&wreq::Client) -> anyhow::Result<wreq::Request>) -> anyhow::Result<T> {
    let ctx = FetchCtxGuard::begin().await;

    let client = ctx.client();
    let req = req(client)?;
    let resp = client.execute(req).await?;
    let json = resp.json::<T>().await?;
    Ok(json)
}
