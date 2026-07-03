use axum::{Json, Router, response::IntoResponse};
use bag_lib::{action::Action, path::{Path, SegmentParseError}, ui::{Component, Gallery, GalleryImage, GalleryImageType}};

use crate::serve::ServeState;

pub async fn render_creators(db: &crate::db::Database, path: &Path<'_>) -> anyhow::Result<impl IntoResponse> {
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

    let mut creators = db.list_fanbox_creators(limit + 1, offset).await?;
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
                to: format!("fanbox/list,creator={}", creator)
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

    Ok(Json(layout))
}

pub async fn render_list(db: &crate::db::Database, path: &Path<'_>) -> anyhow::Result<impl IntoResponse> {
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
                to: format!("fanbox/post/{}", post.id)
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

    Ok(Json(layout))
}

pub fn render() -> Router<ServeState> {
    Router::new().fallback(async move |state: axum::extract::State<ServeState>, uri: axum::http::Uri| {
        let path = uri.path().trim_start_matches('/').trim_end_matches('/');
        tracing::info!("Render request: {}", path);
        let parsed = match Path::try_from(path) {
            Ok(p) => p,
            Err(SegmentParseError::InvalidArgument(arg)) => {
                return (
                    axum::http::StatusCode::BAD_REQUEST,
                    format!("Invalid argument: {}", arg),
                )
                    .into_response();
            }
            Err(SegmentParseError::DecodeError(value)) => {
                return (
                    axum::http::StatusCode::BAD_REQUEST,
                    format!("Decode error: {}", value),
                )
                    .into_response();
            }
        };

        macro_rules! unwrap_resp {
            ($expr:expr) => {
                match $expr {
                    Ok(v) => v.into_response(),
                    Err(e) => {
                        tracing::error!("Error: {:?}", e);
                        return (
                            axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                            format!("Internal server error: {:?}", e),
                        )
                            .into_response();
                    }
                }
            }
        }

        // Routing
        if parsed.0.len() == 1 && parsed.0[0].0 == "creators" {
            // /fanbox/creators
            // Fanbox author list
            return unwrap_resp!(render_creators(&state.db, &parsed).await);
        } else if parsed.0.len() == 1 && parsed.0[0].0 == "list" {
            // /fanbox/list,by=<creator_id>,all=<true|false>
            // Fanbox post list
            return unwrap_resp!(render_list(&state.db, &parsed).await);
        } else if parsed.0.len() == 2 && parsed.0[0].0 == "post" {
            // /post/<id>
            todo!()
        }

        return (
            axum::http::StatusCode::NOT_FOUND,
            format!("Not found: {}", path),
        )
            .into_response();
    })
}

pub fn raw() -> Router<ServeState> {
    Router::new()
}
