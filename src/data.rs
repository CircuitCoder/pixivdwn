use std::collections::HashMap;

use async_stream::try_stream;
use serde::Deserialize;
use serde_repr::Deserialize_repr;

use crate::config::Session;

fn de_str_to_u64<'de, D>(deserializer: D) -> Result<u64, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let s: &str = Deserialize::deserialize(deserializer)?;
    s.parse::<u64>().map_err(serde::de::Error::custom)
}

fn de_str_or_u64_to_u64<'de, D>(deserializer: D) -> Result<u64, D::Error>
where
    D: serde::Deserializer<'de>,
{
    struct StrOrU64;

    impl<'de> serde::de::Visitor<'de> for StrOrU64 {
        type Value = u64;

        fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
            formatter.write_str("string or u64")
        }

        fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
        where
            E: serde::de::Error,
        {
            v.parse::<u64>().map_err(serde::de::Error::custom)
        }

        fn visit_u64<E>(self, v: u64) -> Result<Self::Value, E>
        where
            E: serde::de::Error,
        {
            Ok(v)
        }
    }

    deserializer.deserialize_any(StrOrU64)
}

#[derive(Deserialize_repr, sqlx::Type, Debug, Clone, Copy)]
#[repr(u8)]
pub enum XRestrict {
    Public = 0,
    R18 = 1,
    R18G = 2,
}

#[derive(Deserialize_repr, sqlx::Type, Debug, Clone, Copy)]
#[repr(u8)]
pub enum AIType {
    Unspecified = 0,
    NonAI = 1,
    AI = 2,
}

#[derive(Deserialize, Debug)]
pub struct BookmarkData {
    #[serde(deserialize_with = "de_str_to_u64")]
    id: u64,
    private: bool,
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct BookmarkedWork {
    #[serde(deserialize_with = "de_str_or_u64_to_u64")]
    id: u64,
    title: String,
    tags: Vec<String>,
    x_restrict: XRestrict,

    #[allow(unused)]
    restrict: u8, // TODO: figure out what is this

    #[serde(deserialize_with = "de_str_or_u64_to_u64")]
    user_id: u64,
    user_name: String,

    bookmark_data: BookmarkData,
    create_date: chrono::DateTime<chrono::FixedOffset>,
    update_date: chrono::DateTime<chrono::FixedOffset>,

    #[allow(unused)]
    width: u64,
    #[allow(unused)]
    height: u64,

    is_unlisted: bool,
    is_masked: bool,
    ai_type: AIType,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Bookmarks {
    pub total: usize,
    pub works: Vec<BookmarkedWork>,
    pub bookmark_tags: HashMap<u64, Vec<String>>,
}

impl Bookmarks {
    pub fn into_illusts(self) -> impl Iterator<Item = Illust> {
        let mut tags_map = self.bookmark_tags;
        self.works.into_iter().map(move |work: BookmarkedWork| {
            assert!(!work.is_unlisted || !work.is_masked, "Work cannot be both unlisted and masked");

            let state = if work.is_unlisted {
                tracing::warn!("Unlisted work {:?}", work);
                IllustState::Unlisted
            } else if work.is_masked {
                tracing::warn!("Masked work {}", work.id);
                IllustState::Masked
            } else {
                IllustState::Normal
            };

            let data = if let IllustState::Normal = state {
                IllustData::Fetched(FetchedIllustData {
                    title: work.title,
                    tags: work.tags,
                    author: Illustrator {
                        id: work.user_id,
                        name: work.user_name,
                    },
                    create_date: work.create_date,
                    update_date: work.update_date,
                    x_restrict: work.x_restrict,
                    ai_type: work.ai_type,
                })
            } else {
                IllustData::Unknown
            };

            // Bookmark tags will be kept for unlisted/masked works
            let bookmarked_tags = tags_map.remove(&work.bookmark_data.id).unwrap_or_default();

            Illust {
                id: work.id,
                data,
                state,
                bookmark: Some(IllustBookmarkState {
                    id: work.bookmark_data.id,
                    tags: bookmarked_tags,
                    private: work.bookmark_data.private,
                })
            }
        })
    }
}

#[derive(Deserialize)]
pub struct Response<T> {
    pub error: bool,
    pub message: String,
    pub body: T,
}

impl<T> Response<T> {
    pub fn into_body(self) -> anyhow::Result<T> {
        if self.error {
            Err(anyhow::anyhow!("API error: {}", self.message))
        } else {
            Ok(self.body)
        }
    }
}

pub async fn get_bookmarks_page(user: &Session, tag: Option<&str>, hidden: bool, offset: usize, limit: usize) -> anyhow::Result<Bookmarks> {
    let url = format!(
        "https://www.pixiv.net/ajax/user/{}/illusts/bookmarks",
        user.uid,
    );

    let client = reqwest::Client::new();
    let req = client.get(&url)
        .header("Cookie", format!("PHPSESSID={};", user.pixiv_cookie))
        .header("User-Agent", "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/58.0.3029.110 Safari/537.36")
        .query(&[
            ("tag", tag.unwrap_or("")),
            ("offset", offset.to_string().as_str()),
            ("limit", limit.to_string().as_str()),
            ("rest", if hidden { "hide" } else { "show" }),
            ("lang", "en"),
        ])
        .build()?;
    let resp = client.execute(req).await?;
    let json: Response<Bookmarks> = resp.json().await?;
    json.into_body()
}

// Parsed data

#[derive(sqlx::Type, Debug, Clone, Copy)]
#[repr(u8)]
pub enum IllustState {
    Normal = 0,
    Unlisted = 1,
    Masked = 2,
}

pub struct Illustrator {
    pub id: u64,
    pub name: String,
}

pub struct FetchedIllustData {
    pub title: String,
    pub tags: Vec<String>,
    pub author: Illustrator,
    pub create_date: chrono::DateTime<chrono::FixedOffset>,
    pub update_date: chrono::DateTime<chrono::FixedOffset>,
    pub x_restrict: XRestrict,
    pub ai_type: AIType,
}

pub enum IllustData {
    Unknown,
    Fetched(FetchedIllustData),
}

impl IllustData {
    pub fn as_fetched(&self) -> Option<&FetchedIllustData> {
        match self {
            IllustData::Unknown => None,
            IllustData::Fetched(data) => Some(data),
        }
    }

    pub fn display_title(&self) -> &str {
        match self {
            IllustData::Unknown => "(unknown)",
            IllustData::Fetched(data) => &data.title,
        }
    }
}

pub struct IllustBookmarkState {
    pub tags: Vec<String>,
    pub id: u64, // The id used for ordering bookmarks
    pub private: bool,
}

pub struct Illust {
    pub id: u64,
    pub data: IllustData,
    pub state: IllustState,
    pub bookmark: Option<IllustBookmarkState>,
}

pub async fn get_bookmarks(user: &Session, tag: Option<&str>, hidden: bool) -> impl futures::Stream<Item = anyhow::Result<Illust>> {
    const LIMIT: usize = 48;
    const DELAY_MS: i64 = 2500;
    const DELAY_RANDOM_VAR_MS: i64 = 500;

    let mut offset = 0;

    try_stream! {
        loop {
            let batch = get_bookmarks_page(user, tag, hidden, offset, LIMIT).await?;
            let total = batch.total;
            let batch_size = batch.works.len();

            for illust in batch.into_illusts() {
                yield illust;
            }

            offset += batch_size;
            if offset < total && batch_size == 0 {
                tracing::warn!("Empty batch before reaching end of bookmark list.");
            }

            if offset >= total || batch_size == 0 {
                break;
            }

            let delay = std::time::Duration::from_millis((DELAY_MS + rand::random_range(-DELAY_RANDOM_VAR_MS..=DELAY_RANDOM_VAR_MS)) as u64);
            tracing::info!("Fetched {}/{} bookmarks, sleeping for {:?}...", offset, total, delay);
            tokio::time::sleep(delay).await;
        }
    }
}
