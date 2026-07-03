use axum::{Json, Router, response::{IntoResponse, Response}};
use bag_lib::{action::Action, path::Path, ui::{Component, Gallery, GalleryImage, GalleryImageType}};

use crate::serve::ServeState;

pub async fn render_creators(db: &crate::db::Database, path: Path<'_>) -> anyhow::Result<Response> {
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

    Ok(Json(layout).into_response())
}

pub async fn render_list(db: &crate::db::Database, path: Path<'_>) -> anyhow::Result<Response> {
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

    Ok(Json(layout).into_response())
}

pub async fn render(db: &crate::db::Database, path: Path<'_>) -> anyhow::Result<Response> {
    // Routing
    if path.0.len() == 1 {
        // /fanbox
        // Fanbox author list
        return render_creators(&db, path).await;
    } else if path.0.len() == 2 && path.0[1].0 == "list" {
        // /fanbox/list,by=<creator_id>,all=<true|false>
        // Fanbox post list
        return render_list(&db, path).await;
    } else if path.0.len() == 3 && path.0[1].0 == "post" {
        // /post/<id>
        todo!()
    }

    Ok((
        axum::http::StatusCode::NOT_FOUND,
        format!("Not found: {}", path.to_string()),
    ).into_response())
}

pub fn raw() -> Router<ServeState> {
    Router::new()
}
