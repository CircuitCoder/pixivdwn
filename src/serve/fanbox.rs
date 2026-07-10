use axum::{Json, Router, extract::State, http::HeaderMap, response::{IntoResponse, Response}};
use bag_lib::{action::Action, path::Path, ui::{Component, Gallery, GalleryImage, GalleryImageType, Image, Text, TextVariant}};
use serde::Deserialize;

use crate::serve::ServeState;

async fn render_creators(db: &crate::db::Database, path: Path<'_>) -> anyhow::Result<Response> {
    const DEFAULT_PAGE_SIZE: usize = 100;
    let limit = path
        .last()
        .unwrap()
        .arg("limit")
        .and_then(|s| s.parse::<usize>().ok())
        .unwrap_or(DEFAULT_PAGE_SIZE) as i64;
    let offset = path
        .last()
        .unwrap()
        .arg("offset")
        .and_then(|s| s.parse::<usize>().ok())
        .unwrap_or(0) as i64;
    let all = path
        .last()
        .unwrap()
        .arg("all")
        .and_then(|s| s.parse::<bool>().ok())
        .unwrap_or(false);

    let mut creators = db.list_fanbox_creators(limit + 1, offset, all).await?;
    let is_start = offset == 0;
    let is_end = creators.len() as i64 <= limit;
    if !is_end {
        creators.pop();
    }

    let gallery =  Component::Gallery(Gallery {
        images: creators.into_iter().map(|creator| GalleryImage {
            ty: GalleryImageType::Directory,
            thumbnail: None, // TODO: avatar
            name: creator.clone(),
            action: Some(Action::Navigate {
                to: format!("fanbox/posts,creator={}", creator)
            }),
        }).collect()
    });
    let layout = bag_lib::ui::Layout {
        top: vec![],
        main: vec![
            Component::Box(bag_lib::ui::Box {
                horizontal: false,
                children: vec![
                    Component::Text(Text { content: "All".to_owned(), variant: TextVariant::Body, action: None }),
                ],
                action: Some(Action::Navigate {
                    to: "fanbox/posts".to_owned(),
                }),
            }),
            gallery
        ],
        metadata: vec![],
        left: (!is_start).then(|| {
            let offset_str;
            let offset = if offset > limit {
                offset_str = (offset - limit).to_string();
                Some(offset_str.as_str())
            } else {
                None
            };
            path.with_arg("offset", offset).to_string()
        }),
        right: (!is_end).then(|| {
            let offset_str = (offset + limit).to_string();
            path.with_arg("offset", Some(&offset_str)).to_string()
        }),
    };

    Ok(Json(layout).into_response())
}

async fn render_list(db: &crate::db::Database, path: Path<'_>) -> anyhow::Result<Response> {
    const DEFAULT_PAGE_SIZE: usize = 100;
    let limit = path
        .last()
        .unwrap()
        .arg("limit")
        .and_then(|s| s.parse::<usize>().ok())
        .unwrap_or(DEFAULT_PAGE_SIZE) as i64;
    let offset = path
        .last()
        .unwrap()
        .arg("offset")
        .and_then(|s| s.parse::<usize>().ok())
        .unwrap_or(0) as i64;
    let all = path
        .last()
        .unwrap()
        .arg("all")
        .and_then(|s| s.parse::<bool>().ok())
        .unwrap_or(false);
    let creator = path
        .last()
        .unwrap()
        .arg("creator");

    let mut list = db.list_fanbox_posts(limit + 1, offset, all, creator).await?;
    let is_start = offset == 0;
    let is_end = list.len() as i64 <= limit;
    if !is_end {
        list.pop();
    }

    let gallery = Component::Gallery(Gallery {
        images: list.into_iter().map(|post| GalleryImage {
            ty: GalleryImageType::Directory,
            thumbnail: post.cover_id.map(|id| format!("fanbox/image/{}", id)),
            name: post.title.clone(),
            action: Some(Action::Navigate {
                to: format!("{}/{}", path.to_string(), post.id)
            }),
        }).collect()
    });

    let layout = bag_lib::ui::Layout {
        top: vec![],
        main: vec![gallery],
        metadata: vec![],
        left: (!is_start).then(|| {
            let offset_str;
            let offset = if offset > limit {
                offset_str = (offset - limit).to_string();
                Some(offset_str.as_str())
            } else {
                None
            };
            path.with_arg("offset", offset).to_string()
        }),
        right: (!is_end).then(|| {
            let offset_str = (offset + limit).to_string();
            path.with_arg("offset", Some(&offset_str)).to_string()
        }),
    };

    Ok(Json(layout).into_response())
}

async fn render_post(db: &crate::db::Database, path: Path<'_>) -> anyhow::Result<Response> {
    let Ok(post_id) = path.last().unwrap().0.parse::<u64>() else {
        return Ok((
            axum::http::StatusCode::BAD_REQUEST,
            format!("Invalid post id: {}", path.last().unwrap().0),
        ).into_response());
    };

    let Some(post) = db.get_fanbox_post(post_id).await? else {
        return Ok((
            axum::http::StatusCode::NOT_FOUND,
            format!("Post not found: {}", post_id),
        ).into_response());
    };

    let list_segment = &path.0[1];
    let all = list_segment
        .arg("all")
        .and_then(|s| s.parse::<bool>().ok())
        .unwrap_or(false);
    let creator = list_segment.arg("creator");
    let (prev, next) = db.get_fanbox_post_siblings(&post, all, creator).await?;
    let parent_path = path.parent().unwrap().to_string();

    async fn render_file(db: &crate::db::Database, id: &str) -> anyhow::Result<Vec<Component>> {
        let Some((file, ext)) = db.get_fanbox_file(id).await? else {
            return Ok(vec![
                Component::Text(Text {content:format!("File not found: {}", id), variant: TextVariant::Hint, action: None })
            ]);
        };

        let mime = mime_guess::from_ext(&ext).first_or_octet_stream();
        if mime.type_().as_str() == "video" || mime.type_().as_str() == "image" {
            return Ok(vec![
                Component::Image(Image { resource: format!("fanbox/file/{}", id), mime: Some(mime.to_string()) })
            ]);
        }

        if ext != "zip" {
            return Ok(vec![
                Component::Text(Text {content:format!("File not supported: {}.{}", id, ext), variant: TextVariant::Hint, action: None })
            ]);
        }

        if !std::path::Path::new(&file).is_absolute() {
            return Ok(vec![
                Component::Text(Text {content:format!("File path is not absolute: {}", file), variant: TextVariant::Hint, action: None })
            ]);
        }

        let files = tokio::task::spawn_blocking(move || -> anyhow::Result<Vec<String>> {
            let mut zip = zip::ZipArchive::new(std::fs::File::open(&file)?)?;
            let mut files = Vec::new();
            for i in 0..zip.len() {
                let file = zip.by_index(i)?;
                if file.is_dir() {
                    continue;
                }
                files.push(file.name().to_string());
            }
            alphanumeric_sort::sort_str_slice(&mut files);
            Ok(files)
        }).await??;

        let mut ret = vec![
            Component::Text(Text {content:format!("File {} contains {} files", id, files.len()), variant: TextVariant::Hint, action: None })
        ];
        // FIXME: actually renders the file recursively
        for file in files {
            let encoded = urlencoding::encode(&file);
            ret.push(Component::Text(Text {content:format!("File: {}", file), variant: TextVariant::Hint, action: None }));
            ret.push(Component::Image(Image { resource: format!("fanbox/file/{}?subpath={encoded}", id), mime: None }));
        }
        Ok(ret)
    }

    let main = match post.body {
        None => vec![Component::Text(Text {content:"Not Fetched".to_string(), variant: TextVariant::Hint, action: None })],
        Some(crate::db::FanboxPostBody::SimpleOrLegacy(content)) => {
            // TODO: fetch all files
            let images = db.list_fanbox_post_images(post_id).await?;
            let files = db.list_fanbox_post_files(post_id).await?;
            let mut ret = vec![
                Component::Text(Text {content, variant: TextVariant::Body, action: None }),
            ];
            ret.extend(images.into_iter().map(|img| {
                Component::Image(Image { resource: format!("fanbox/image/{}", img.id), mime: None })
            }));
            for file in files {
                ret.extend(render_file(db, &file.id).await?);
            }
            ret
        },
        // TODO: change back to single component
        // Add box for files
        Some(crate::db::FanboxPostBody::Rich(body)) => futures::future::join_all(body.into_iter().map(async |e| match e {
            crate::data::fanbox::FetchPostBlock::Paragraph { text }
                => vec![Component::Text(Text {content:text, variant: TextVariant::Body, action: None })],
            crate::data::fanbox::FetchPostBlock::Header { text }
                => vec![Component::Text(Text {content:text, variant: TextVariant::Title, action: None })],
            crate::data::fanbox::FetchPostBlock::File { file_id }
                => render_file(db, &file_id).await.unwrap_or(vec![Component::Text(Text {content:format!("File not found: {}", file_id), variant: TextVariant::Hint, action: None })]),
            crate::data::fanbox::FetchPostBlock::Image { image_id }
                => vec![Component::Image(Image { resource: format!("fanbox/image/{}", image_id), mime: None })],
            crate::data::fanbox::FetchPostBlock::Embed { embed_id: _, content }
                => vec![Component::Text(Text {content:format!("Embed: {}", serde_json::to_string(&content).unwrap()), variant: TextVariant::Hint, action: None })],
            crate::data::fanbox::FetchPostBlock::UrlEmbed { url_embed_id: _, content }
                => vec![Component::Text(Text {content:format!("URL Embed: {}", serde_json::to_string(&content).unwrap()), variant: TextVariant::Hint, action: None })],
        })).await.into_iter().flatten().collect()
    };


    let layout = bag_lib::ui::Layout {
        top: vec![],
        main,
        metadata: vec![
            Component::Text(Text {content: post.title, variant: TextVariant::Title, action: None }),
            Component::Text(Text {content: "Creator".to_owned(), variant: TextVariant::Hint, action: None }),
            Component::Text(Text {content: post.creator_id.clone(), variant: TextVariant::Body, action: Some(Action::Navigate {
                to: format!("fanbox/posts,creator={}", post.creator_id)
            })}),
            Component::Text(Text {content: "ID".to_owned(), variant: TextVariant::Hint, action: None }),
            Component::Text(Text {content: post.id.to_string(), variant: TextVariant::Body, action: None }),
            Component::Text(Text {content: "Published".to_owned(), variant: TextVariant::Hint, action: None }),
            Component::Text(Text {content: post.published_datetime.to_rfc3339(), variant: TextVariant::Body, action: None }),
            Component::Text(Text {content: "Updated".to_owned(), variant: TextVariant::Hint, action: None }),
            Component::Text(Text {content: post.updated_datetime.to_rfc3339(), variant: TextVariant::Body, action: None }),
        ],
        left: prev.map(|id| format!("{}/{}", parent_path, id)),
        right: next.map(|id| format!("{}/{}", parent_path, id)),
    };

    Ok(Json(layout).into_response())
}

pub async fn render(db: &crate::db::Database, path: Path<'_>) -> anyhow::Result<Response> {
    // Routing
    if path.0.len() == 1 {
        // /fanbox
        // Fanbox author list
        return render_creators(&db, path).await;
    } else if path.0.len() == 2 && path.0[1].0 == "posts" {
        // /fanbox/posts,creator=<creator_id>,all=<true|false>
        // Fanbox post list
        return render_list(&db, path).await;
    } else if path.0.len() == 3 && path.0[1].0 == "posts" {
        // /fanbox/posts,creator=<creator_id>,all=<true|false>/<id>
        return render_post(&db, path).await;
    } else if path.0.len() >= 3 && path.0[1].0 == "archive" {
        // /archive/<id>/:/<subpath...>
        // Images never have subpaths
        todo!()
    }

    Ok((
        axum::http::StatusCode::NOT_FOUND,
        format!("Not found: {}", path.to_string()),
    ).into_response())
}

async fn fanbox_image(state: State<ServeState>, axum::extract::Path(id): axum::extract::Path<String>, headers: HeaderMap) -> Response {
    let path = match state.db.get_fanbox_image(&id).await {
        Ok(Some((path, _))) => path,
        Ok(None) => return (
            axum::http::StatusCode::NOT_FOUND,
            format!("Image not found: {}", id),
        ).into_response(),
        Err(e) => return (
            axum::http::StatusCode::INTERNAL_SERVER_ERROR,
            format!("Error fetching image: {}", e),
        ).into_response(),
    };

    tracing::info!("Serving fanbox image {} from path {}", id, path.to_string());
    state.fanbox_fs.handle(&path, &headers).await
}

#[derive(Deserialize)]
struct FileQuery {
    subpath: Option<String>,
}

async fn fanbox_file(
    state: State<ServeState>,
    axum::extract::Path(id): axum::extract::Path<String>,
    axum::extract::Query(query): axum::extract::Query<FileQuery>,
    headers: HeaderMap
) -> Response {
    let path = match state.db.get_fanbox_file(&id).await {
        Ok(Some((path, _))) => path,
        Ok(None) => return (
            axum::http::StatusCode::NOT_FOUND,
            format!("File not found: {}", id),
        ).into_response(),
        Err(e) => return (
            axum::http::StatusCode::INTERNAL_SERVER_ERROR,
            format!("Error fetching image: {}", e),
        ).into_response(),
    };

    let joined = if let Some(subpath) = query.subpath {
        path.to_owned() + "/:/" + &subpath
    } else {
        path.to_owned()
    };
    tracing::info!("Serving fanbox image {} from path {}", id, joined);
    state.fanbox_fs.handle(&joined, &headers).await
}

pub fn raw() -> Router<ServeState> {
    Router::new()
        .route("/image/{id}", axum::routing::get(fanbox_image))
        .route("/file/{id}", axum::routing::get(fanbox_file))
}
