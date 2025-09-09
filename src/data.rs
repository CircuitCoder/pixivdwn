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
    #[serde(deserialize_with = "de_str_to_u64")]
    id: u64,
    title: String,
    tags: Vec<String>,
    x_restrict: XRestrict,
    restruct: u8, // TODO: figure out what is this

    #[serde(deserialize_with = "de_str_to_u64")]
    user_id: u64,
    user_name: String,

    bookmark_data: BookmarkData,

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
    pub bookmark_tags: HashMap<String, Vec<String>>,
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
