use std::collections::HashMap;

use async_stream::try_stream;
use serde::{Deserialize, Serialize, de::IgnoredAny};
use serde_repr::Deserialize_repr;

use crate::{
    config::Session,
    data::{RequestArgumenter, RequestExt},
};

#[derive(Deserialize_repr, sqlx::Type, Debug, Clone, Copy)]
#[repr(u8)]
pub enum IllustType {
    Illustration = 0,
    Manga = 1,
    Ugoira = 2,
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
    #[serde(deserialize_with = "super::de_str_to_u64")]
    id: u64,
    private: bool,
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
#[allow(unused)]
pub struct DetailedTag {
    pub tag: String,
    pub locked: bool,
    pub deletable: bool,
    #[serde(deserialize_with = "super::de_str_to_u64")]
    pub user_id: u64,
    pub user_name: String,
    pub romaji: Option<String>,
    pub translation: Option<HashMap<String, String>>,
}

#[derive(Deserialize, Debug)]
#[serde(untagged)]
pub enum Tags {
    Brief(Vec<String>),
    #[serde(rename_all = "camelCase")]
    #[allow(unused)]
    Detailed {
        #[serde(deserialize_with = "super::de_str_to_u64")]
        author_id: u64,
        is_locked: bool,
        writable: bool,
        tags: Vec<DetailedTag>,
    },
}

impl Tags {
    pub fn tag_names(&self) -> impl Iterator<Item = &str> {
        let ret: Box<dyn Iterator<Item = _>> = match self {
            Tags::Brief(tags) => Box::new(tags.iter().map(|s| s.as_str())),
            Tags::Detailed { tags, .. } => Box::new(tags.iter().map(|t| t.tag.as_str())),
        };
        ret
    }
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct FetchWorkBrief {
    #[serde(deserialize_with = "super::de_str_or_u64_to_u64")]
    id: u64,
    title: String,
    tags: Tags,
    x_restrict: XRestrict,

    illust_type: IllustType,
    page_count: u64,

    #[allow(unused)]
    restrict: u8, // TODO: figure out what is this

    #[serde(deserialize_with = "super::de_str_or_u64_to_u64")]
    user_id: u64,
    user_name: String,
    user_account: Option<String>,

    bookmark_data: BookmarkData,
    create_date: chrono::DateTime<chrono::FixedOffset>,
    #[serde(alias = "uploadDate")] // Detail field
    update_date: chrono::DateTime<chrono::FixedOffset>,

    #[allow(unused)]
    width: u64,
    #[allow(unused)]
    height: u64,

    #[serde(default)] // For single illust API
    is_unlisted: bool,
    #[serde(default)] // For single illust API
    is_masked: bool,
    ai_type: AIType,
}

impl Into<Illust> for FetchWorkBrief {
    fn into(self) -> Illust {
        assert!(
            !self.is_unlisted || !self.is_masked,
            "self cannot be both unlisted and masked"
        );

        let state = if self.is_unlisted {
            tracing::warn!("Unlisted self {:?}", self);
            IllustState::Unlisted
        } else if self.is_masked {
            tracing::warn!("Masked self {}", self.id);
            IllustState::Masked
        } else {
            IllustState::Normal
        };

        let data = if let IllustState::Normal = state {
            IllustData::Simple(IllustDataSimple {
                title: self.title,
                tags: self.tags,
                author: Illustrator {
                    id: self.user_id,
                    name: self.user_name,
                    account: self.user_account,
                },
                create_date: self.create_date,
                update_date: self.update_date,
                x_restrict: self.x_restrict,
                ai_type: self.ai_type,

                illust_type: self.illust_type,
                page_count: self.page_count,
            })
        } else {
            IllustData::Unknown
        };

        Illust {
            id: self.id,
            data,
            state,
            bookmark: Some(IllustBookmarkState {
                id: self.bookmark_data.id,
                tags: IllustBookmarkTags::Unknown,
                private: self.bookmark_data.private,
            }),
        }
    }
}

#[derive(Deserialize, Debug)]
pub struct PageUrls {
    #[serde(alias = "thumb_mini")]
    #[allow(unused)]
    pub mini: String,
    #[allow(unused)]
    pub thumb: Option<String>,
    #[allow(unused)]
    pub small: String,
    #[allow(unused)]
    pub regular: String,

    pub original: String,
}

#[derive(Deserialize, Debug)]
pub struct Page {
    pub urls: PageUrls,
    pub width: u64,
    pub height: u64,
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct FetchWorkDetail {
    #[serde(deserialize_with = "super::de_str_to_u64")]
    pub illust_id: u64,
    pub illust_title: String,
    pub illust_comment: String,
    pub description: String,

    #[serde(flatten)]
    pub brief: FetchWorkBrief,

    #[allow(unused)]
    pub bookmark_count: u64,
    #[allow(unused)]
    pub like_count: u64,
    #[allow(unused)]
    pub comment_count: u64,
    #[allow(unused)]
    pub response_count: u64,
    #[allow(unused)]
    pub view_count: u64,

    #[allow(unused)]
    pub urls: PageUrls,

    pub is_howto: bool,
    pub is_original: bool,
}

impl Into<Illust> for FetchWorkDetail {
    fn into(self) -> Illust {
        let mut illust: Illust = self.brief.into();
        match illust.data {
            IllustData::Unknown => {}
            IllustData::Simple(brief) => {
                assert_eq!(self.illust_comment, self.description);
                assert_eq!(self.illust_id, illust.id);
                assert_eq!(self.illust_title, brief.title);

                let extra = IllustDataDetail {
                    desc: self.description,
                    is_howto: self.is_howto,
                    is_original: self.is_original,
                };
                illust.data = IllustData::Detailed(brief, extra);
            }
            IllustData::Detailed(_, _) => unreachable!(),
        }
        illust
    }
}

/// Deserialize bookmarkTags, which is either a map of numbers to list of strings, or an empty ARRAY
fn de_bookmark_tags<'de, D>(deserializer: D) -> Result<HashMap<u64, Vec<String>>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    struct BookmarkTagsVisitor;

    impl<'de> serde::de::Visitor<'de> for BookmarkTagsVisitor {
        type Value = HashMap<u64, Vec<String>>;

        fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
            formatter.write_str("map of numbers to list of strings or an empty array")
        }

        fn visit_map<M>(self, mut access: M) -> Result<Self::Value, M::Error>
        where
            M: serde::de::MapAccess<'de>,
        {
            let mut map = HashMap::new();
            while let Some((key, value)) = access.next_entry::<&str, Vec<String>>()? {
                let key = key.parse::<u64>().map_err(serde::de::Error::custom)?;
                map.insert(key, value);
            }
            Ok(map)
        }

        fn visit_seq<A>(self, mut seq: A) -> Result<Self::Value, A::Error>
        where
            A: serde::de::SeqAccess<'de>,
        {
            if seq.next_element::<IgnoredAny>()?.is_some() {
                return Err(serde::de::Error::custom("Expected empty array"));
            }
            Ok(HashMap::new())
        }
    }

    deserializer.deserialize_any(BookmarkTagsVisitor)
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Bookmarks {
    pub total: usize,
    pub works: Vec<FetchWorkBrief>,
    #[serde(deserialize_with = "de_bookmark_tags")]
    pub bookmark_tags: HashMap<u64, Vec<String>>,
}

impl Bookmarks {
    pub fn into_illusts(self) -> impl Iterator<Item = Illust> {
        let mut tags_map = self.bookmark_tags;
        self.works.into_iter().map(move |work: FetchWorkBrief| {
            // Bookmark tags will be kept for unlisted/masked works
            let mut illust: Illust = work.into();
            if let Some(ref mut bookmark) = illust.bookmark {
                let bookmarked_tags = tags_map.remove(&bookmark.id).unwrap_or_default();
                bookmark.tags = IllustBookmarkTags::Known(bookmarked_tags);
            }
            illust
        })
    }
}

#[derive(Deserialize, Serialize)]
pub struct UgoiraFrame {
    pub file: String,
    pub delay: u64,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UgoiraMeta {
    #[allow(unused)]
    pub src: String,
    pub original_src: String,
    #[serde(rename = "mime_type")]
    pub mime_type: String,
    pub frames: Vec<UgoiraFrame>,
}

#[derive(Deserialize)]
pub struct Response<T> {
    pub error: bool,
    pub message: String,
    pub body: Option<T>,
}

impl<T> Response<T> {
    pub fn into_body(self) -> anyhow::Result<T> {
        if self.error {
            Err(anyhow::anyhow!("API error: {}", self.message))
        } else {
            self.body
                .ok_or_else(|| anyhow::anyhow!("No body in response"))
        }
    }
}

pub struct PixivRequest<'a>(pub &'a Session);

impl RequestArgumenter for PixivRequest<'_> {
    fn argument(self, req: wreq::RequestBuilder) -> anyhow::Result<wreq::RequestBuilder> {
        let pixiv_session = self
            .0
            .pixiv
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("Pixiv session is required"))?;
        Ok(req
            .header("Cookie", format!("PHPSESSID={};", pixiv_session.cookie))
            .header("User-Agent", "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/58.0.3029.110 Safari/537.36"))
    }
}

async fn get_bookmarks_page(
    session: &Session,
    tag: Option<&str>,
    hidden: bool,
    offset: usize,
    limit: usize,
) -> anyhow::Result<Bookmarks> {
    let pixiv_session = session
        .pixiv
        .as_ref()
        .ok_or_else(|| anyhow::anyhow!("Pixiv session is required"))?;

    let url = format!(
        "https://www.pixiv.net/ajax/user/{}/illusts/bookmarks",
        pixiv_session.uid,
    );

    let req = |client: &wreq::Client| {
        Ok(client
            .get(&url)
            .prepare_with(PixivRequest(session))?
            .query(&[
                ("tag", tag.unwrap_or("")),
                ("offset", offset.to_string().as_str()),
                ("limit", limit.to_string().as_str()),
                ("rest", if hidden { "hide" } else { "show" }),
                ("lang", "en"),
            ])
            .build()?)
    };
    let json: Response<Bookmarks> = crate::fetch::fetch(req).await?;
    json.into_body()
}

// Parsed data

#[derive(clap::ValueEnum, sqlx::Type, Debug, Clone, Copy)]
#[repr(u8)]
pub enum IllustState {
    Normal = 0,
    Unlisted = 1,
    Masked = 2,
}

#[derive(Debug)]
pub struct Illustrator {
    pub id: u64,
    pub name: String,
    pub account: Option<String>,
}

#[derive(Debug)]
pub struct IllustDataSimple {
    pub title: String,
    pub tags: Tags,
    pub author: Illustrator,
    pub create_date: chrono::DateTime<chrono::FixedOffset>,
    pub update_date: chrono::DateTime<chrono::FixedOffset>,
    pub x_restrict: XRestrict,
    pub ai_type: AIType,

    pub illust_type: IllustType,
    pub page_count: u64,
}

#[derive(Debug)]
pub struct IllustDataDetail {
    pub desc: String,
    pub is_howto: bool,
    pub is_original: bool,
}

#[derive(Debug)]
pub enum IllustData {
    Unknown,
    Simple(IllustDataSimple),
    Detailed(IllustDataSimple, IllustDataDetail),
}

impl IllustData {
    pub fn as_simple(&self) -> Option<&IllustDataSimple> {
        match self {
            IllustData::Unknown => None,
            IllustData::Simple(data) => Some(data),
            IllustData::Detailed(data, _) => Some(data),
        }
    }

    pub fn as_detail(&self) -> Option<&IllustDataDetail> {
        match self {
            IllustData::Detailed(_, detail) => Some(detail),
            _ => None,
        }
    }

    pub fn display_title(&self) -> &str {
        self.as_simple()
            .map(|d| d.title.as_str())
            .unwrap_or("(unknown)")
    }
}

#[derive(Debug)]
pub enum IllustBookmarkTags {
    Known(Vec<String>),
    Unknown,
}

#[derive(Debug)]
pub struct IllustBookmarkState {
    pub id: u64, // The id used for ordering bookmarks
    pub private: bool,
    pub tags: IllustBookmarkTags,
}

#[derive(Debug)]
pub struct Illust {
    pub id: u64,
    pub data: IllustData,
    pub state: IllustState,
    pub bookmark: Option<IllustBookmarkState>,
}

pub async fn get_bookmarks(
    session: &Session,
    tag: Option<&str>,
    mut offset: usize,
    hidden: bool,
) -> impl futures::Stream<Item = anyhow::Result<Illust>> {
    const LIMIT: usize = 48;

    try_stream! {
        loop {
            let batch = get_bookmarks_page(session, tag, hidden, offset, LIMIT).await?;
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

            tracing::info!("Fetched {}/{} bookmarks", offset, total);
        }
    }
}

pub async fn get_illust(session: &Session, illust_id: u64) -> anyhow::Result<Illust> {
    let url = format!("https://www.pixiv.net/ajax/illust/{}", illust_id);

    let req = |client: &wreq::Client| {
        Ok(client
            .get(&url)
            .prepare_with(PixivRequest(session))?
            .build()?)
    };
    let json: Response<FetchWorkDetail> = crate::fetch::fetch(req).await?;
    let detail = json.into_body()?;
    Ok(detail.into())
}

pub async fn get_illust_pages(session: &Session, illust_id: u64) -> anyhow::Result<Vec<Page>> {
    let url = format!("https://www.pixiv.net/ajax/illust/{}/pages", illust_id);

    let req = |client: &wreq::Client| {
        Ok(client
            .get(&url)
            .prepare_with(PixivRequest(session))?
            .build()?)
    };
    let json: Response<Vec<Page>> = crate::fetch::fetch(req).await?;
    let pages = json.into_body()?;
    Ok(pages)
}

pub async fn get_illust_ugoira_meta(
    session: &Session,
    illust_id: u64,
) -> anyhow::Result<UgoiraMeta> {
    let url = format!(
        "https://www.pixiv.net/ajax/illust/{}/ugoira_meta",
        illust_id
    );

    let req = |client: &wreq::Client| {
        Ok(client
            .get(&url)
            .prepare_with(PixivRequest(session))?
            .build()?)
    };
    let json: Response<UgoiraMeta> = crate::fetch::fetch(req).await?;
    let meta = json.into_body()?;
    Ok(meta)
}
