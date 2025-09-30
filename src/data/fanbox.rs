use async_stream::try_stream;
use serde::Deserialize;
use serde_repr::Deserialize_repr;

use crate::config::Session;

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct LinkedPixivUser {
    #[serde(deserialize_with = "super::de_str_to_u64")]
    user_id: u64,
    name: String,
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct FetchPostCover {
    r#type: String,
    url: String,
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct FetchPost {
    #[serde(deserialize_with = "super::de_str_to_u64")]
    id: u64,
    title: String,
    fee_required: u64,
    published_datetime: chrono::DateTime<chrono::FixedOffset>,
    updated_datetime: chrono::DateTime<chrono::FixedOffset>,
    tags: Vec<String>,

    is_liked: bool,
    like_count: usize,
    is_commenting_restricted: bool,
    comment_count: usize,
    is_restricted: bool,

    user: Option<LinkedPixivUser>,
    creator_id: String,
    has_adult_content: bool,
    cover: Option<FetchPostCover>,
    excerpt: String,
    is_pinned: bool,
}

trait RequestExt {
    fn prepare_with(self, cookie: &str) -> Self;
}

impl RequestExt for reqwest::RequestBuilder {
    fn prepare_with(self, cookie: &str) -> Self {
        self.header("Cookie", format!("FANBOXSESSID={};", cookie))
            .header("Origin", "https://www.fanbox.cc")
            .header("Referer", "https://www.fanbox.cc/")
            .header("User-Agent", "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/58.0.3029.110 Safari/537.36")
            .query(&[
                ("lang", "en"),
            ])
    }
}

#[derive(Deserialize)]
#[serde(untagged)]
pub enum Response<T> {
    Errored {
        error: String,
    },
    Success {
        body: T,
    }
}

impl<T> Response<T> {
    pub fn into_body(self) -> anyhow::Result<T> {
        match self {
            Response::Errored { error } => Err(anyhow::anyhow!("Fanbox API error: {}", error)),
            Response::Success { body } => Ok(body),
        }
    }
}


pub async fn get_author_paginates(session: &Session, author_id: &str) -> anyhow::Result<Vec<String>> {
    let fanbox_session = session.fanbox.as_ref()
        .ok_or_else(|| anyhow::anyhow!("Fanbox session is required"))?;
    let url = format!(
        "https://api.fanbox.cc/post.paginateCreator?creatorId={}&sort=newest",
       author_id 
    );

    let client = reqwest::Client::new();
    let req = client.get(&url)
        .prepare_with(fanbox_session.cookie.as_str())
        .build()?;
    let resp = client.execute(req).await?;
    let text = resp.text().await?;
    tracing::info!("Fanbox paginate response: {}", text);
    let json: Response<Vec<String>> = serde_json::from_str(&text)?;
    json.into_body()
}

pub fn fetch_author_posts(session: &Session, author_id: &str) -> impl futures::Stream<Item = anyhow::Result<FetchPost>> {
    const DELAY_MS: i64 = 2500;
    const DELAY_RANDOM_VAR_MS: i64 = 500;

    try_stream! {
        let fanbox_session = session.fanbox.as_ref()
            .ok_or_else(|| anyhow::anyhow!("Fanbox session is required"))?;

        let paginates = get_author_paginates(session, author_id).await?;
        for page_url in paginates {
            // FIXME: assert page_url format
            tracing::info!("Fetching: {}", page_url);

            let client = reqwest::Client::new();
            let req = client.get(&page_url)
                .prepare_with(fanbox_session.cookie.as_str())
                .build()?;
            let resp = client.execute(req).await?;
            let posts: Response<Vec<FetchPost>> = resp.json().await?;
            for post in posts.into_body()? {
                yield post;
            }

            let delay = std::time::Duration::from_millis((DELAY_MS + rand::random_range(-DELAY_RANDOM_VAR_MS..=DELAY_RANDOM_VAR_MS)) as u64);
            tokio::time::sleep(delay).await;
        }
    }
}
