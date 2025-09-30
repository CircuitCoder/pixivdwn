use std::collections::HashMap;

use async_stream::try_stream;
use serde::{Deserialize, Serialize};

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
    pub id: u64,
    pub title: String,
    pub fee_required: u64,
    pub published_datetime: chrono::DateTime<chrono::FixedOffset>,
    pub updated_datetime: chrono::DateTime<chrono::FixedOffset>,
    pub tags: Vec<String>,

    pub is_liked: bool,
    pub like_count: usize,
    pub is_commenting_restricted: bool,
    pub comment_count: usize,
    pub is_restricted: bool,

    pub user: Option<LinkedPixivUser>,
    pub creator_id: String,
    pub has_adult_content: bool,
    pub cover: Option<FetchPostCover>,
    pub excerpt: String,
    pub is_pinned: bool,
}

#[derive(Deserialize, Serialize, Debug)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum FetchPostBlock {
    #[serde(rename = "p")]
    Paragraph { text: String },

    #[serde(rename_all = "camelCase")]
    File { file_id: String },

    #[serde(rename_all = "camelCase")]
    Image { image_id: String },
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct FetchPostImage {
    pub id: String,
    pub extension: String,
    pub width: u64,
    pub height: u64,
    pub original_url: String,
    pub thumbnail_url: String,
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct FetchPostFile {
    pub id: String,
    pub name: String,
    pub extension: String,
    pub size: u64,
    pub url: String,
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct FetchPostBody {
    pub blocks: Vec<FetchPostBlock>,
    pub image_map: HashMap<String, FetchPostImage>,
    pub file_map: HashMap<String, FetchPostFile>,
    pub embed_map: HashMap<String, serde_json::Value>,
    pub url_embed_map: HashMap<String, serde_json::Value>,
}

#[derive(Deserialize, Debug)]
pub struct FetchPostDetail {
    #[serde(flatten)]
    pub post: FetchPost,

    pub body: FetchPostBody,
}

trait RequestExt : Sized {
    fn prepare_with(self, cookie: &Session) -> anyhow::Result<Self>;
}

impl RequestExt for wreq::RequestBuilder {
    fn prepare_with(self, session: &Session) -> anyhow::Result<Self> {
        let updated = if let Some(ref full) = session.fanbox_full {
            self.header("Cookie", full)
        } else if let Some(ref cookie) = session.fanbox {
            self.header("Cookie", format!("FANBOXSESSID={};", cookie.cookie))
        } else {
            return Err(anyhow::anyhow!("Fanbox session is required"));
        };

        let updated = updated
            .header("Origin", "https://www.fanbox.cc")
            .header("Referer", "https://www.fanbox.cc/")
            .header("User-Agent", "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/58.0.3029.110 Safari/537.36");
        Ok(updated)
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
    let url = format!(
        "https://api.fanbox.cc/post.paginateCreator?creatorId={}&sort=newest",
       author_id 
    );

    let client = wreq::Client::new();
    let req = client.get(&url)
        .prepare_with(session)?
        .emulation(wreq_util::Emulation::Chrome140)
        .build()?;
    let resp = client.execute(req).await?;
    let text = resp.text().await?;
    tracing::debug!("Fanbox paginate response: {}", text);
    let json: Response<Vec<String>> = serde_json::from_str(&text)?;
    json.into_body()
}

pub fn fetch_author_posts(session: &Session, author_id: &str) -> impl futures::Stream<Item = anyhow::Result<FetchPost>> {
    const DELAY_MS: i64 = 2500;
    const DELAY_RANDOM_VAR_MS: i64 = 500;

    try_stream! {
        let paginates = get_author_paginates(session, author_id).await?;
        for page_url in paginates {
            // FIXME: assert page_url format
            tracing::info!("Fetching: {}", page_url);

            let client = wreq::Client::new();
            let req = client.get(&page_url)
                .prepare_with(session)?
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

pub async fn fetch_post(session: &Session, post_id: u64) -> anyhow::Result<FetchPostDetail> {
    let url = format!(
        "https://api.fanbox.cc/post.info?postId={}",
       post_id 
    );

    let client = wreq::Client::new();
    let req = client.get(&url)
        .prepare_with(session)?
        .emulation(wreq_util::Emulation::Chrome140)
        .build()?;
    let resp = client.execute(req).await?;
    let text = resp.text().await?;
    tracing::debug!("Fanbox post response: {}", text);
    let json: Response<FetchPostDetail> = serde_json::from_str(&text)?;
    json.into_body()
}
