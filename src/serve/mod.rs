use std::path::PathBuf;

use axum::{Router, extract::State, http::Uri, response::{IntoResponse, Response}};
use bag_fs::fs::FsHandler;
use bag_lib::path::{Path, SegmentParseError};
use tower_http::cors::{Any, CorsLayer};

pub mod fanbox;

#[derive(Clone)]
pub struct ServeState {
    db: crate::db::Database,
    #[allow(unused)]
    fanbox_base: PathBuf,
    fanbox_fs: FsHandler,
}

macro_rules! unwrap_resp {
    ($expr:expr) => {
        match $expr {
            Ok(v) => return v.into_response(),
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


pub fn build(session: &crate::config::Session, db: crate::db::Database) -> anyhow::Result<Router> {
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    let render = Router::new().fallback(async move |state: State<ServeState>, uri: Uri| -> Response {
        let path = uri.path().trim_start_matches('/').trim_end_matches('/');
        tracing::info!("Render request: {}", path);
        let parsed = match Path::try_from(path) {
            Ok(p) => p,
            Err(SegmentParseError::InvalidArgument(arg)) => {
                return (
                    axum::http::StatusCode::BAD_REQUEST,
                    format!("Invalid argument: {}", arg),
                ).into_response();
            }
            Err(SegmentParseError::DecodeError(value)) => {
                return (
                    axum::http::StatusCode::BAD_REQUEST,
                    format!("Decode error: {}", value),
                ).into_response();
            }
        };

        if parsed.0.len() > 0 && parsed.0[0].0 == "fanbox" {
            unwrap_resp!(fanbox::render(&state.db, parsed).await);
        }

        (
            axum::http::StatusCode::NOT_FOUND,
            format!("Not found: {}", path.to_string()),
        ).into_response()
    });

    let fanbox_base = session.fanbox_base_dir.as_ref().ok_or_else(|| anyhow::anyhow!("Fanbox Base dir must be set"))?;

    Ok(Router::new()
        .nest("/v1/raw/fanbox", fanbox::raw())
        .nest("/v1/render", render)
        .with_state(ServeState {
            db,
            fanbox_base: fanbox_base.clone(),
            fanbox_fs: FsHandler::new(fanbox_base.clone()),
        })
        .layer(cors))
}
