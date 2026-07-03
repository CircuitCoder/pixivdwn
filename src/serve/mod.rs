use axum::Router;
use tower_http::cors::{Any, CorsLayer};

pub mod fanbox;

#[derive(Clone)]
pub struct ServeState {
    db: crate::db::Database,
}

pub fn build(db: crate::db::Database) -> Router {
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    Router::new()
        .nest("/v1/raw/fanbox", fanbox::raw())
        .nest("/v1/render/fanbox", fanbox::render())
        .with_state(ServeState { db })
        .layer(cors)
}
