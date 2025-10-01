use std::collections::HashMap;

use async_stream::try_stream;
use serde::{Deserialize, Serialize};

use crate::{
    config::Session,
    data::{RequestArgumenter, RequestExt},
};

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
#[allow(unused)]
pub struct LinkedPixivUser {
    #[serde(deserialize_with = "super::de_str_to_u64")]
    pub user_id: u64,
    pub name: String,
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
#[allow(unused)]
pub struct FetchPostCover {
    r#type: String,
    url: String,
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
#[allow(unused)]
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
#[serde(tag = "type", rename_all = "snake_case")]
pub enum FetchPostBlock {
    #[serde(rename = "p")]
    Paragraph {
        text: String,
    },

    Header {
        text: String,
    },

    #[serde(rename_all = "camelCase")]
    File {
        file_id: String,
    },

    #[serde(rename_all = "camelCase")]
    Image {
        image_id: String,
    },

    #[serde(rename_all = "camelCase")]
    UrlEmbed {
        url_embed_id: String,

        #[serde(skip_deserializing)]
        content: Option<FetchPostUrlEmbed>,
    },
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct FetchPostImage {
    pub id: String,
    pub extension: String,
    pub width: u64,
    pub height: u64,
    pub original_url: String,
    #[allow(unused)]
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

#[derive(Deserialize, Serialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct FetchPostUrlEmbed {
    #[serde(skip_serializing)]
    pub id: String,
    pub r#type: String,
    pub html: String,
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct FetchPostBodyRichRaw {
    pub blocks: Vec<FetchPostBlock>,
    pub image_map: HashMap<String, FetchPostImage>,
    pub file_map: HashMap<String, FetchPostFile>,
    pub embed_map: HashMap<String, serde_json::Value>,
    pub url_embed_map: HashMap<String, FetchPostUrlEmbed>,
}

#[derive(Debug, Deserialize)]
#[serde(try_from = "FetchPostBodyRichRaw")]
pub struct FetchPostBodyRich {
    pub blocks: Vec<FetchPostBlock>,
    pub images: Vec<(usize, FetchPostImage)>,
    pub files: Vec<(usize, FetchPostFile)>,
}

#[derive(thiserror::Error, Debug)]
pub enum FetchPostBodyConversionError {
    #[error("Unmatched image ID: {0}")]
    UnmatchedImageId(String),
    #[error("Unmatched file ID: {0}")]
    UnmatchedFileId(String),
    #[error("Unmatched url_embed ID: {0}")]
    UnmatchedUrlEmbedId(String),
    #[error(
        "Extra unmapped items exist: image: {image:?} file: {file:?} embed: {embed:?} url_embed: {url_embed:?}"
    )]
    Extra {
        image: bool,
        file: bool,
        embed: bool,
        url_embed: bool,
    },
}

impl TryFrom<FetchPostBodyRichRaw> for FetchPostBodyRich {
    type Error = FetchPostBodyConversionError;

    fn try_from(mut raw: FetchPostBodyRichRaw) -> Result<Self, Self::Error> {
        tracing::debug!("Converting rich body: {:#?}", raw);
        let mut images = Vec::new();
        let mut files = Vec::new();

        for (idx, block) in raw.blocks.iter_mut().enumerate() {
            match block {
                FetchPostBlock::Image { image_id } => {
                    let inner = raw.image_map.remove(image_id).ok_or_else(|| {
                        FetchPostBodyConversionError::UnmatchedImageId(image_id.clone())
                    })?;
                    images.push((idx, inner));
                }
                FetchPostBlock::File { file_id } => {
                    let inner = raw.file_map.remove(file_id).ok_or_else(|| {
                        FetchPostBodyConversionError::UnmatchedFileId(file_id.clone())
                    })?;
                    files.push((idx, inner));
                }
                FetchPostBlock::UrlEmbed {
                    url_embed_id,
                    content,
                } => {
                    let inner = raw.url_embed_map.remove(url_embed_id).ok_or_else(|| {
                        FetchPostBodyConversionError::UnmatchedUrlEmbedId(url_embed_id.clone())
                    })?;
                    *content = Some(inner);
                }
                _ => {}
            }
        }

        let extra_embed = raw.embed_map.len() != 0;
        let extra_url_embed = raw.url_embed_map.len() != 0;
        let extra_image = raw.image_map.len() != 0;
        let extra_file = raw.file_map.len() != 0;
        if extra_embed || extra_url_embed || extra_image || extra_file {
            return Err(FetchPostBodyConversionError::Extra {
                embed: extra_embed,
                url_embed: extra_url_embed,
                image: extra_image,
                file: extra_file,
            });
        }

        tracing::debug!(
            "Conversion successful: images: {:#?}, files: {:#?}",
            images,
            files
        );

        Ok(Self {
            blocks: raw.blocks,
            images,
            files,
        })
    }
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct FetchPostBodySimple {
    pub text: String,
    #[serde(default)]
    pub images: Vec<FetchPostImage>,
    #[serde(default)]
    pub files: Vec<FetchPostFile>,
}

#[derive(Deserialize, Debug)]
#[serde(untagged)]
pub enum FetchPostBody {
    Rich(FetchPostBodyRich),
    Simple(FetchPostBodySimple),
}

impl FetchPostBody {
    pub fn images<'a>(&'a self) -> Box<dyn Iterator<Item = (usize, &'a FetchPostImage)> + 'a> {
        match self {
            FetchPostBody::Rich(rich) => Box::new(rich.images.iter().map(|(idx, img)| (*idx, img))),
            FetchPostBody::Simple(simple) => Box::new(simple.images.iter().enumerate()),
        }
    }

    pub fn files<'a>(&'a self) -> Box<dyn Iterator<Item = (usize, &'a FetchPostFile)> + 'a> {
        match self {
            FetchPostBody::Rich(rich) => {
                Box::new(rich.files.iter().map(|(idx, file)| (*idx, file)))
            }
            FetchPostBody::Simple(simple) => {
                let img_len = simple.images.len();
                Box::new(
                    simple
                        .files
                        .iter()
                        .enumerate()
                        .map(move |(i, file)| (i + img_len, file)),
                )
            }
        }
    }

    pub fn text_repr(&self) -> anyhow::Result<String> {
        let txt = match self {
            FetchPostBody::Rich(rich) => serde_json::to_string(&rich.blocks)?,
            FetchPostBody::Simple(simple) => simple.text.clone(),
        };
        Ok(txt)
    }

    pub fn is_rich(&self) -> bool {
        matches!(self, FetchPostBody::Rich(_))
    }
}

#[derive(Deserialize, Debug)]
pub struct FetchPostDetail {
    #[serde(flatten)]
    pub post: FetchPost,

    pub body: Option<FetchPostBody>,
}

pub struct FanboxRequest<'a>(pub &'a Session);

impl<'a> RequestArgumenter for FanboxRequest<'a> {
    fn argument(self, req: wreq::RequestBuilder) -> anyhow::Result<wreq::RequestBuilder> {
        let updated = if let Some(ref full) = self.0.fanbox_full {
            req.header("Cookie", full)
        } else if let Some(ref cookie) = self.0.fanbox {
            req.header("Cookie", format!("FANBOXSESSID={};", cookie.cookie))
        } else {
            return Err(anyhow::anyhow!("Fanbox session is required"));
        };

        let updated = updated
            .header("Origin", "https://www.fanbox.cc")
            .header("Referer", "https://www.fanbox.cc/")
            .emulation(wreq_util::Emulation::Chrome140);
        Ok(updated)
    }
}

#[derive(Deserialize)]
#[serde(untagged)]
pub enum Response<T> {
    Errored { error: String },
    Success { body: T },
}

impl<T> Response<T> {
    pub fn into_body(self) -> anyhow::Result<T> {
        match self {
            Response::Errored { error } => Err(anyhow::anyhow!("Fanbox API error: {}", error)),
            Response::Success { body } => Ok(body),
        }
    }
}

pub async fn get_author_paginates(
    session: &Session,
    author_id: &str,
) -> anyhow::Result<Vec<String>> {
    let url = format!(
        "https://api.fanbox.cc/post.paginateCreator?creatorId={}&sort=newest",
        author_id
    );

    let json: Response<Vec<String>> = crate::fetch::fetch(|client| {
        Ok(client
            .get(&url)
            .prepare_with(FanboxRequest(session))?
            .build()?)
    })
    .await?;

    json.into_body()
}

pub fn fetch_author_posts(
    session: &Session,
    author_id: &str,
    skip_pages: usize,
) -> impl futures::Stream<Item = anyhow::Result<FetchPost>> {
    try_stream! {
        let paginates = get_author_paginates(session, author_id).await?;
        for (page, url) in paginates.iter().enumerate() {
            if page < skip_pages {
                tracing::info!("Skipping page {}/{}", page + 1, paginates.len());
                continue;
            }

            // FIXME: assert url format
            tracing::info!("Fetching page {}/{}", page + 1, paginates.len());

            let posts: Response<Vec<FetchPost>> = crate::fetch::fetch(|client| {
                Ok(client.get(url).prepare_with(FanboxRequest(session))?.build()?)
            }).await?;
            for post in posts.into_body()? {
                yield post;
            }
        }
    }
}

pub async fn fetch_post(session: &Session, post_id: u64) -> anyhow::Result<FetchPostDetail> {
    let url = format!("https://api.fanbox.cc/post.info?postId={}", post_id);

    let json: Response<FetchPostDetail> = crate::fetch::fetch(|client| {
        Ok(client
            .get(&url)
            .prepare_with(FanboxRequest(session))?
            .build()?)
    })
    .await?;
    json.into_body()
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct SupportedCreator {
    #[serde(deserialize_with = "super::de_str_to_u64")]
    pub id: u64,
    pub creator_id: String,
    pub user: Option<LinkedPixivUser>,
    pub has_adult_content: bool,

    // Supporting plan
    pub fee: u64,
    pub title: String,
    pub description: String,
}

pub async fn fetch_supporting_list(session: &Session) -> anyhow::Result<Vec<SupportedCreator>> {
    let url = "https://api.fanbox.cc/plan.listSupporting";
    let json: Response<Vec<SupportedCreator>> = crate::fetch::fetch(|client| {
        Ok(client
            .get(url)
            .prepare_with(FanboxRequest(session))?
            .build()?)
    })
    .await?;
    json.into_body()
}
