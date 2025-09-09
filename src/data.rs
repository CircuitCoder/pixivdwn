use std::collections::HashMap;

use serde::Deserialize;
use serde_repr::Deserialize_repr;

use crate::config::User;

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

#[derive(Deserialize_repr)]
#[repr(u8)]
pub enum XRestrict {
    Public = 0,
    R18 = 1,
    R18G = 2,
}

#[derive(Deserialize_repr)]
#[repr(u8)]
pub enum AIType {
    NonAI = 1,
    AI = 2,
}

#[derive(Deserialize)]
pub struct BookmarkData {
    #[serde(deserialize_with = "de_str_to_u64")]
    id: u64,
    private: bool,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BookmarkedWork {
    #[serde(deserialize_with = "de_str_or_u64_to_u64")]
    id: u64,
    title: String,
    tags: Vec<String>,
    x_restrict: XRestrict,
    restrict: u8, // TODO: figure out what is this

    #[serde(deserialize_with = "de_str_or_u64_to_u64")]
    user_id: u64,
    user_name: String,

    bookmark_data: BookmarkData,
    create_date: chrono::DateTime<chrono::FixedOffset>,
    update_date: chrono::DateTime<chrono::FixedOffset>,

    width: u64,
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
    pub fn into_illusts(&mut self) -> impl Iterator<Item = Illust> {
        let mut tags_map = std::mem::take(&mut self.bookmark_tags);
        self.works.drain(..).map(move |work: BookmarkedWork| {
            let state = if work.is_unlisted {
                IllustState::Unlisted
            } else if work.is_masked {
                IllustState::Masked
            } else {
                IllustState::Normal
            };
            let data = IllustData::Fetched {
                title: work.title,
                tags: work.tags,
                user: Illustrator {
                    id: work.user_id,
                    name: work.user_name,
                },
                create_date: work.create_date,
                update_date: work.update_date,
                x_restrict: work.x_restrict,
            };
            let bookmarked_tags = tags_map.remove(&work.id).unwrap();
            Illust {
                data,
                state,
                bookmarked_tags,
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

pub async fn get_bookmarks_page(user: User, tag: Option<&str>, hidden: bool, offset: usize, limit: usize) -> anyhow::Result<Bookmarks> {
    let url = format!(
        "https://www.pixiv.net/ajax/user/{}/illusts/bookmarks",
        user.id,
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

pub enum IllustState {
    Normal,
    Unlisted,
    Masked,
}

pub struct Illustrator {
    id: u64,
    name: String,
}

pub enum IllustData {
    Unknown,
    Fetched {
        title: String,
        tags: Vec<String>,
        user: Illustrator,
        create_date: chrono::DateTime<chrono::FixedOffset>,
        update_date: chrono::DateTime<chrono::FixedOffset>,
        x_restrict: XRestrict,
    }
}

pub struct Illust {
    data: IllustData,
    state: IllustState,
    bookmarked_tags: Vec<String>,
}
