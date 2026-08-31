use std::{
    borrow::Cow,
    io::{BufReader, Cursor, Read, Seek},
    path::PathBuf,
};

use axum::{
    Json, Router,
    extract::State,
    http::HeaderMap,
    response::{IntoResponse, Response},
};
use bag_fs::{
    file::{ArchiveEntry, ArchiveOpen, File},
    render::{
        ArchiveRenderParams, render_archive as render_archive_listing, render_archive_file,
        render_archive_password,
    },
    serve::{FileThunk, RenderContext, finalize_range, read_range},
    thumb::{extract_thumbnail_img, extract_thumbnail_video},
};
use bag_lib::{
    action::Action,
    path::{Path, Segment, SegmentParseError},
    ui::{
        Box as UiBox, Component, Gallery, GalleryImage, GalleryImageType, Image, Layout,
        LayoutOrAction, Text, TextVariant,
    },
};
use serde::Deserialize;

use crate::serve::ServeState;

const DEFAULT_ARCHIVE_PAGE_SIZE: usize = 100;
const THUMBNAIL_MAX_DIMENSION: u32 = 150;

trait ReadSeek: Read + Seek {}

impl<T: Read + Seek> ReadSeek for T {}

struct ReadableFile {
    file: Box<dyn ReadSeek + Send>,
    size: u64,
}

enum ArchiveView {
    File(Vec<ArchiveEntry>),
    Directory(Vec<ArchiveEntry>),
}

async fn open_readable(thunk: FileThunk<'_>) -> bag_fs::Result<ReadableFile> {
    match thunk {
        FileThunk::Fs(file) => {
            let size = file.metadata()?.len();
            Ok(ReadableFile {
                file: Box::new(file),
                size,
            })
        }
        FileThunk::Nested(file, ty, path) => {
            let path = path.to_static();
            tokio::task::spawn_blocking(move || {
                File::Fs(file).descend(ty, &path, |opened| match opened {
                    ArchiveOpen::File { mut file, size, .. } => {
                        let mut contents = Vec::new();
                        file.read_to_end(&mut contents)?;
                        Ok(ReadableFile {
                            file: Box::new(Cursor::new(contents)),
                            size,
                        })
                    }
                    ArchiveOpen::Directory(_) => Err(bag_fs::Error::NotFound),
                })
            })
            .await?
        }
    }
}

async fn validate_nested(thunk: FileThunk<'_>) -> bag_fs::Result<()> {
    let FileThunk::Nested(file, ty, path) = thunk else {
        return Ok(());
    };
    let path = path.to_static();
    tokio::task::spawn_blocking(move || {
        File::Fs(file).descend(ty, &path, |opened| match opened {
            ArchiveOpen::File { mut file, .. } => {
                std::io::copy(&mut file, &mut std::io::sink())?;
                Ok(())
            }
            ArchiveOpen::Directory(_) => Err(bag_fs::Error::NotFound),
        })
    })
    .await?
}

async fn open_archive_view(thunk: FileThunk<'_>) -> bag_fs::Result<ArchiveView> {
    let FileThunk::Nested(file, ty, path) = thunk else {
        return Err(bag_fs::Error::InvalidPath);
    };
    let path = path.to_static();
    tokio::task::spawn_blocking(move || {
        File::Fs(file).descend(ty, &path, |opened| {
            Ok(match opened {
                ArchiveOpen::File { siblings, .. } => ArchiveView::File(siblings),
                ArchiveOpen::Directory(entries) => ArchiveView::Directory(entries),
            })
        })
    })
    .await?
}

fn owned_segment(segment: &Segment<'_>) -> Segment<'static> {
    Segment(
        Cow::Owned(segment.name().to_owned()),
        segment
            .1
            .iter()
            .map(|(key, value)| {
                (
                    Cow::Owned(key.as_ref().to_owned()),
                    Cow::Owned(value.as_ref().to_owned()),
                )
            })
            .collect(),
    )
}

fn stored_fs_path(base: &std::path::Path, stored_path: &str) -> PathBuf {
    let stored_path = PathBuf::from(stored_path);
    if stored_path.is_absolute() {
        stored_path
    } else {
        base.join(stored_path)
    }
}

fn file_request_path(id: &str, subpath: Option<&str>) -> Result<Path<'static>, SegmentParseError> {
    let mut segments = vec![Segment(Cow::Owned(id.to_owned()), Default::default())];
    if let Some(subpath) = subpath {
        let parsed = Path::try_from(subpath)?;
        if parsed.first().is_none_or(|segment| segment.name() != ":") {
            segments.push(Segment(Cow::Borrowed(":"), Default::default()));
        }
        segments.extend(parsed.segments().iter().map(owned_segment));
    }
    Ok(Path(Cow::Owned(segments)))
}

fn split_request_path(path: &Path<'_>) -> (String, Path<'static>) {
    let (db_path, subpath) = path.split_subpath(":");
    (
        db_path.to_bare_string(),
        subpath.map_or_else(Path::empty, |path| path.to_static()),
    )
}

fn media_filename(subpath: &Path<'_>, fallback: String) -> String {
    subpath
        .last()
        .map(|segment| segment.name().to_owned())
        .unwrap_or(fallback)
}

fn fs_error_response(error: bag_fs::Error) -> Response {
    bag_fs::serve::error_to_resp(error)
}

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

    let gallery = Component::Gallery(Gallery {
        images: creators
            .into_iter()
            .map(|creator| GalleryImage {
                ty: GalleryImageType::Directory,
                thumbnail: None, // TODO: avatar
                name: creator.clone(),
                action: Some(Action::Navigate {
                    to: format!("fanbox/posts,creator={}", creator),
                }),
            })
            .collect(),
    });
    let layout = bag_lib::ui::Layout {
        top: vec![],
        main: vec![
            Component::Box(bag_lib::ui::Box {
                horizontal: false,
                children: vec![Component::Text(Text {
                    content: "All".to_owned(),
                    variant: TextVariant::Body,
                    action: None,
                })],
                action: Some(Action::Navigate {
                    to: "fanbox/posts".to_owned(),
                }),
            }),
            gallery,
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
    let creator = path.last().unwrap().arg("creator");

    let mut list = db
        .list_fanbox_posts(limit + 1, offset, all, creator)
        .await?;
    let is_start = offset == 0;
    let is_end = list.len() as i64 <= limit;
    if !is_end {
        list.pop();
    }

    let gallery = Component::Gallery(Gallery {
        images: list
            .into_iter()
            .map(|post| GalleryImage {
                ty: GalleryImageType::Directory,
                thumbnail: post
                    .cover_id
                    .map(|id| format!("fanbox/thumbnail/image/{}", id)),
                name: post.title.clone(),
                action: Some(Action::Navigate {
                    to: format!("{}/{}", path.to_string(), post.id),
                }),
            })
            .collect(),
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
        )
            .into_response());
    };

    let Some(post) = db.get_fanbox_post(post_id).await? else {
        return Ok((
            axum::http::StatusCode::NOT_FOUND,
            format!("Post not found: {}", post_id),
        )
            .into_response());
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
        let Some((_, ext, name)) = db.get_fanbox_file(id).await? else {
            return Ok(vec![Component::Text(Text {
                content: format!("File not found: {}", id),
                variant: TextVariant::Hint,
                action: None,
            })]);
        };

        let mime = mime_guess::from_ext(&ext).first_or_octet_stream();
        if mime.type_().as_str() == "video" || mime.type_().as_str() == "image" {
            return Ok(vec![Component::Image(Image {
                resource: format!("fanbox/file/{}", id),
                mime: Some(mime.to_string()),
            })]);
        }

        if !ext.eq_ignore_ascii_case("zip") {
            return Ok(vec![Component::Text(Text {
                content: format!("File not supported: {}.{}", id, ext),
                variant: TextVariant::Hint,
                action: None,
            })]);
        }

        Ok(vec![Component::Box(UiBox {
            horizontal: false,
            children: vec![Component::Text(Text {
                content: name,
                variant: TextVariant::Body,
                action: None,
            })],
            action: Some(Action::Navigate {
                to: format!("fanbox/archive/{id}"),
            }),
        })])
    }

    let main = match post.body {
        None => vec![Component::Text(Text {
            content: "Not Fetched".to_string(),
            variant: TextVariant::Hint,
            action: None,
        })],
        Some(crate::db::FanboxPostBody::SimpleOrLegacy(content)) => {
            // TODO: fetch all files
            let images = db.list_fanbox_post_images(post_id).await?;
            let files = db.list_fanbox_post_files(post_id).await?;
            let mut ret = vec![Component::Text(Text {
                content,
                variant: TextVariant::Body,
                action: None,
            })];
            ret.extend(images.into_iter().map(|img| {
                Component::Image(Image {
                    resource: format!("fanbox/image/{}", img.id),
                    mime: None,
                })
            }));
            for file in files {
                ret.extend(render_file(db, &file.id).await?);
            }
            ret
        }
        // TODO: change back to single component
        // Add box for files
        Some(crate::db::FanboxPostBody::Rich(body)) => {
            futures::future::join_all(body.into_iter().map(async |e| {
                match e {
                    crate::data::fanbox::FetchPostBlock::Paragraph { text } => {
                        vec![Component::Text(Text {
                            content: text,
                            variant: TextVariant::Body,
                            action: None,
                        })]
                    }
                    crate::data::fanbox::FetchPostBlock::Header { text } => {
                        vec![Component::Text(Text {
                            content: text,
                            variant: TextVariant::Title,
                            action: None,
                        })]
                    }
                    crate::data::fanbox::FetchPostBlock::File { file_id } => {
                        render_file(db, &file_id)
                            .await
                            .unwrap_or(vec![Component::Text(Text {
                                content: format!("File not found: {}", file_id),
                                variant: TextVariant::Hint,
                                action: None,
                            })])
                    }
                    crate::data::fanbox::FetchPostBlock::Image { image_id } => {
                        vec![Component::Image(Image {
                            resource: format!("fanbox/image/{}", image_id),
                            mime: None,
                        })]
                    }
                    crate::data::fanbox::FetchPostBlock::Embed {
                        embed_id: _,
                        content,
                    } => vec![Component::Text(Text {
                        content: format!("Embed: {}", serde_json::to_string(&content).unwrap()),
                        variant: TextVariant::Hint,
                        action: None,
                    })],
                    crate::data::fanbox::FetchPostBlock::UrlEmbed {
                        url_embed_id: _,
                        content,
                    } => vec![Component::Text(Text {
                        content: format!("URL Embed: {}", serde_json::to_string(&content).unwrap()),
                        variant: TextVariant::Hint,
                        action: None,
                    })],
                }
            }))
            .await
            .into_iter()
            .flatten()
            .collect()
        }
    };

    let layout = bag_lib::ui::Layout {
        top: vec![],
        main,
        metadata: vec![
            Component::Text(Text {
                content: post.title,
                variant: TextVariant::Title,
                action: None,
            }),
            Component::Text(Text {
                content: "Creator".to_owned(),
                variant: TextVariant::Hint,
                action: None,
            }),
            Component::Text(Text {
                content: post.creator_id.clone(),
                variant: TextVariant::Body,
                action: Some(Action::Navigate {
                    to: format!("fanbox/posts,creator={}", post.creator_id),
                }),
            }),
            Component::Text(Text {
                content: "ID".to_owned(),
                variant: TextVariant::Hint,
                action: None,
            }),
            Component::Text(Text {
                content: post.id.to_string(),
                variant: TextVariant::Body,
                action: None,
            }),
            Component::Text(Text {
                content: "Published".to_owned(),
                variant: TextVariant::Hint,
                action: None,
            }),
            Component::Text(Text {
                content: post.published_datetime.to_rfc3339(),
                variant: TextVariant::Body,
                action: None,
            }),
            Component::Text(Text {
                content: "Updated".to_owned(),
                variant: TextVariant::Hint,
                action: None,
            }),
            Component::Text(Text {
                content: post.updated_datetime.to_rfc3339(),
                variant: TextVariant::Body,
                action: None,
            }),
        ],
        left: prev.map(|id| format!("{}/{}", parent_path, id)),
        right: next.map(|id| format!("{}/{}", parent_path, id)),
    };

    Ok(Json(layout).into_response())
}

fn append_path_segment<'a>(path: &Path<'a>, name: &str) -> Path<'a> {
    let mut segments = path.segments().to_vec();
    segments.push(Segment(Cow::Owned(name.to_owned()), Default::default()));
    Path(Cow::Owned(segments))
}

fn archive_redirect(path: &Path<'_>) -> Response {
    Json(LayoutOrAction::Action(Action::Navigate {
        to: append_path_segment(path, ":").to_string(),
    }))
    .into_response()
}

fn is_archive_path(path: &str) -> bool {
    mime_guess::from_path(path).first_or_octet_stream() == "application/zip"
}

fn archive_file_resource(path: &Path<'_>, id: &str) -> String {
    let tail = Path::new(Cow::Borrowed(&path.segments()[3..]));
    let encoded = urlencoding::encode(&tail.to_string()).into_owned();
    format!("fanbox/file/{id}?subpath={encoded}")
}

fn archive_thumbnail_resource(path: Path<'_>, id: &str) -> Option<String> {
    let tail = Path::new(Cow::Borrowed(&path.segments()[3..]));
    let encoded = urlencoding::encode(&tail.to_string()).into_owned();
    Some(format!("fanbox/thumbnail/file/{id}?subpath={encoded}"))
}

fn sort_archive_listing(listing: &mut [ArchiveEntry]) {
    listing.sort_by(|left, right| {
        right
            .is_dir
            .cmp(&left.is_dir)
            .then_with(|| alphanumeric_sort::compare_str(&left.name, &right.name))
    });
}

fn archive_file_layout(path: &Path<'_>, id: &str, siblings: Vec<ArchiveEntry>) -> Layout {
    let mut layout = render_archive_file(path, siblings, vec![]);
    if let Some(Component::Image(image)) = layout.main.first_mut() {
        image.resource = archive_file_resource(path, id);
    }
    layout
}

async fn render_archive(
    db: &crate::db::Database,
    base: &std::path::Path,
    path: Path<'_>,
) -> anyhow::Result<Response> {
    let Some(request_path) = path.suffix(2) else {
        return Ok((axum::http::StatusCode::BAD_REQUEST, "Missing file ID").into_response());
    };
    let (id, subpath) = split_request_path(&request_path);
    if id.is_empty() {
        return Ok((axum::http::StatusCode::BAD_REQUEST, "Missing file ID").into_response());
    }
    let Some((stored_path, ext, _)) = db.get_fanbox_file(&id).await? else {
        return Ok((axum::http::StatusCode::NOT_FOUND, "File not found").into_response());
    };
    let fs_path = stored_fs_path(base, &stored_path);

    if subpath.is_empty() {
        if is_archive_path(&format!("file.{ext}")) {
            return Ok(archive_redirect(&path));
        }
        return Ok((
            axum::http::StatusCode::BAD_REQUEST,
            "File is not a supported archive",
        )
            .into_response());
    }
    if subpath.first().is_none_or(|segment| segment.name() != ":") {
        return Ok((
            axum::http::StatusCode::BAD_REQUEST,
            "Archive path must begin with an archive marker",
        )
            .into_response());
    }

    let path = path.to_static();
    let response = bag_fs::serve::serve(&fs_path, &subpath, &HeaderMap::new(), async move |ctx| {
        let result = match open_archive_view(ctx.file).await {
            Ok(ArchiveView::Directory(mut listing)) => {
                sort_archive_listing(&mut listing);
                LayoutOrAction::Layout(render_archive_listing(
                    ArchiveRenderParams {
                        path: path.borrow(),
                        default_page_size: DEFAULT_ARCHIVE_PAGE_SIZE,
                        thumbnail: |path: Path<'_>| archive_thumbnail_resource(path, &id),
                    },
                    listing,
                    vec![],
                ))
            }
            Ok(ArchiveView::File(mut siblings)) => {
                if path
                    .last()
                    .is_some_and(|segment| is_archive_path(segment.name()))
                {
                    LayoutOrAction::Action(Action::Navigate {
                        to: append_path_segment(&path, ":").to_string(),
                    })
                } else {
                    sort_archive_listing(&mut siblings);
                    LayoutOrAction::Layout(archive_file_layout(&path, &id, siblings))
                }
            }
            Err(bag_fs::Error::ArchivePassword(suffix_len)) => {
                let offset = path
                    .len()
                    .checked_sub(suffix_len)
                    .ok_or(bag_fs::Error::InvalidPath)?;
                if path
                    .segments()
                    .get(offset)
                    .is_none_or(|segment| segment.name() != ":")
                {
                    return Err(bag_fs::Error::InvalidPath);
                }
                LayoutOrAction::Layout(render_archive_password(&path, offset, vec![]))
            }
            Err(error) => return Err(error),
        };
        Ok(Json(result).into_response())
    })
    .await;

    Ok(match response {
        Ok(response) => response,
        Err(error) => fs_error_response(error),
    })
}

pub async fn render(
    db: &crate::db::Database,
    base: &std::path::Path,
    path: Path<'_>,
) -> anyhow::Result<Response> {
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
        return render_archive(db, base, path).await;
    }

    Ok((
        axum::http::StatusCode::NOT_FOUND,
        format!("Not found: {}", path.to_string()),
    )
        .into_response())
}

async fn raw_file_response(
    base: &std::path::Path,
    path: &Path<'_>,
    filename: String,
    headers: &HeaderMap,
) -> Response {
    let mime = mime_guess::from_path(&filename).first_or_octet_stream();
    let request_headers = headers.clone();

    match bag_fs::serve::serve(
        base,
        path,
        headers,
        async move |RenderContext { file, etag }| {
            let source = open_readable(file).await?;
            let size = source.size;
            let range_etag = etag.clone();
            let (body, range) = tokio::task::spawn_blocking(move || {
                read_range(source.file, &request_headers, &range_etag, size)
            })
            .await??;
            finalize_range(body, range, mime, &filename, &etag, size).await
        },
    )
    .await
    {
        Ok(response) => response,
        Err(error) => fs_error_response(error),
    }
}

async fn fanbox_image(
    state: State<ServeState>,
    axum::extract::Path(id): axum::extract::Path<String>,
    headers: HeaderMap,
) -> Response {
    let request_path = file_request_path(&id, None).expect("a file ID is a valid path segment");
    let (id, subpath) = split_request_path(&request_path);
    let (stored_path, ext) = match state.db.get_fanbox_image(&id).await {
        Ok(Some(source)) => source,
        Ok(None) => {
            return (
                axum::http::StatusCode::NOT_FOUND,
                format!("Image not found: {}", id),
            )
                .into_response();
        }
        Err(e) => {
            return (
                axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                format!("Error fetching image: {}", e),
            )
                .into_response();
        }
    };
    let fs_path = stored_fs_path(&state.fanbox_base, &stored_path);
    let filename = media_filename(&subpath, format!("{id}.{ext}"));

    tracing::info!(
        "Serving fanbox image {} from path {}",
        id,
        fs_path.display()
    );
    raw_file_response(&fs_path, &subpath, filename, &headers).await
}

#[derive(Deserialize)]
struct FileQuery {
    subpath: Option<String>,
}

async fn fanbox_file(
    state: State<ServeState>,
    axum::extract::Path(id): axum::extract::Path<String>,
    axum::extract::Query(query): axum::extract::Query<FileQuery>,
    headers: HeaderMap,
) -> Response {
    let request_path = match file_request_path(&id, query.subpath.as_deref()) {
        Ok(path) => path,
        Err(error) => {
            return (
                axum::http::StatusCode::BAD_REQUEST,
                format!("Invalid archive subpath: {error:?}"),
            )
                .into_response();
        }
    };
    let (id, subpath) = split_request_path(&request_path);
    let (stored_path, ext, _) = match state.db.get_fanbox_file(&id).await {
        Ok(Some(source)) => source,
        Ok(None) => {
            return (
                axum::http::StatusCode::NOT_FOUND,
                format!("File not found: {}", id),
            )
                .into_response();
        }
        Err(e) => {
            return (
                axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                format!("Error fetching image: {}", e),
            )
                .into_response();
        }
    };
    let fs_path = stored_fs_path(&state.fanbox_base, &stored_path);
    let filename = media_filename(&subpath, format!("{id}.{ext}"));

    tracing::info!("Serving fanbox file {} from path {}", id, fs_path.display());
    raw_file_response(&fs_path, &subpath, filename, &headers).await
}

async fn fanbox_image_thumbnail(
    state: State<ServeState>,
    axum::extract::Path(id): axum::extract::Path<String>,
    headers: HeaderMap,
) -> Response {
    let request_path = file_request_path(&id, None).expect("a file ID is a valid path segment");
    let (id, subpath) = split_request_path(&request_path);
    let (stored_path, ext, cached) = match state.db.get_fanbox_image_thumbnail(&id).await {
        Ok(Some(source)) => source,
        Ok(None) => {
            return (
                axum::http::StatusCode::NOT_FOUND,
                format!("Image not found: {id}"),
            )
                .into_response();
        }
        Err(error) => return thumbnail_internal_error("query image thumbnail", &id, error),
    };
    let fs_path = stored_fs_path(&state.fanbox_base, &stored_path);
    let db = state.db.clone();
    let request_headers = headers.clone();
    let filename = format!("{id}.{ext}");
    let result = bag_fs::serve::serve(
        &fs_path,
        &subpath,
        &headers,
        async move |RenderContext { file, etag }| {
            let thumbnail = if let Some(thumbnail) = cached {
                validate_nested(file).await?;
                thumbnail
            } else {
                let source = open_readable(file).await?;
                let source = BufReader::new(source.file);
                let thumbnail = match extract_thumbnail_img(source, THUMBNAIL_MAX_DIMENSION).await {
                    Ok(thumbnail) => thumbnail,
                    Err(error) => {
                        tracing::error!(
                            "Failed to generate thumbnail for image {id}.{ext}: {error}"
                        );
                        return Ok((
                            axum::http::StatusCode::NOT_FOUND,
                            "Failed to generate thumbnail",
                        )
                            .into_response());
                    }
                };
                if let Err(error) = db.update_fanbox_image_thumbnail(&id, &thumbnail).await {
                    tracing::error!("Failed to cache thumbnail for image {id}: {error}");
                }
                thumbnail
            };

            thumbnail_response(thumbnail, &filename, &etag, &request_headers).await
        },
    )
    .await;

    match result {
        Ok(response) => response,
        Err(error) => fs_error_response(error),
    }
}

async fn fanbox_file_thumbnail(
    state: State<ServeState>,
    axum::extract::Path(id): axum::extract::Path<String>,
    axum::extract::Query(query): axum::extract::Query<FileQuery>,
    headers: HeaderMap,
) -> Response {
    let request_path = match file_request_path(&id, query.subpath.as_deref()) {
        Ok(path) => path,
        Err(error) => {
            return (
                axum::http::StatusCode::BAD_REQUEST,
                format!("Invalid archive subpath: {error:?}"),
            )
                .into_response();
        }
    };
    let (id, subpath) = split_request_path(&request_path);
    let cache_subpath = (!subpath.is_empty()).then(|| subpath.to_bare_string());
    let (stored_path, ext, cached) = match state
        .db
        .get_fanbox_file_thumbnail(&id, cache_subpath.as_deref())
        .await
    {
        Ok(Some(source)) => source,
        Ok(None) => {
            return (
                axum::http::StatusCode::NOT_FOUND,
                format!("File not found: {id}"),
            )
                .into_response();
        }
        Err(error) => return thumbnail_internal_error("query file thumbnail", &id, error),
    };
    let fs_path = stored_fs_path(&state.fanbox_base, &stored_path);
    let media_path = media_filename(&subpath, format!("file.{ext}"));
    if is_archive_path(&media_path) || subpath.last().is_some_and(|segment| segment.name() == ":") {
        return (
            axum::http::StatusCode::BAD_REQUEST,
            "Archives do not have thumbnails",
        )
            .into_response();
    }

    let db = state.db.clone();
    let request_headers = headers.clone();
    let cache_subpath_for_log = cache_subpath.clone();
    let result = bag_fs::serve::serve(
        &fs_path,
        &subpath,
        &headers,
        async move |RenderContext { file, etag }| {
            let thumbnail = if let Some(thumbnail) = cached {
                validate_nested(file).await?;
                thumbnail
            } else {
                let source = open_readable(file).await?;
                let source = BufReader::new(source.file);
                let mime = mime_guess::from_path(&media_path).first_or_octet_stream();
                let generated = if mime.type_().as_str() == "video" {
                    extract_thumbnail_video(source, THUMBNAIL_MAX_DIMENSION).await
                } else {
                    extract_thumbnail_img(source, THUMBNAIL_MAX_DIMENSION).await
                };
                let thumbnail = match generated {
                    Ok(thumbnail) => thumbnail,
                    Err(error) => {
                        tracing::error!(
                            "Failed to generate thumbnail for file {id} ({}): {error}",
                            cache_subpath_for_log.as_deref().unwrap_or("<whole file>")
                        );
                        return Ok((
                            axum::http::StatusCode::NOT_FOUND,
                            "Failed to generate thumbnail",
                        )
                            .into_response());
                    }
                };

                if let Err(error) = db
                    .update_fanbox_file_thumbnail(&id, cache_subpath.as_deref(), &thumbnail)
                    .await
                {
                    tracing::error!(
                        "Failed to cache thumbnail for file {id} ({}): {error}",
                        cache_subpath.as_deref().unwrap_or("<whole file>")
                    );
                }
                thumbnail
            };

            thumbnail_response(thumbnail, &media_path, &etag, &request_headers).await
        },
    )
    .await;

    match result {
        Ok(response) => response,
        Err(error) => fs_error_response(error),
    }
}

async fn thumbnail_response(
    thumbnail: Vec<u8>,
    filename: &str,
    etag: &str,
    headers: &HeaderMap,
) -> bag_fs::Result<Response> {
    let size = thumbnail.len() as u64;
    let headers = headers.clone();
    let etag_for_range = etag.to_owned();
    let (body, range) = tokio::task::spawn_blocking(move || {
        read_range(Cursor::new(thumbnail), &headers, &etag_for_range, size)
    })
    .await??;
    finalize_range(
        body,
        range,
        "image/webp".parse().expect("WebP MIME type is valid"),
        filename,
        etag,
        size,
    )
    .await
}

fn thumbnail_internal_error(context: &str, id: &str, error: anyhow::Error) -> Response {
    tracing::error!("Failed to {context} for {id}: {error}");
    (
        axum::http::StatusCode::INTERNAL_SERVER_ERROR,
        "Internal server error",
    )
        .into_response()
}

pub fn raw() -> Router<ServeState> {
    Router::new()
        .route("/image/{id}", axum::routing::get(fanbox_image))
        .route("/file/{id}", axum::routing::get(fanbox_file))
        .route(
            "/thumbnail/image/{id}",
            axum::routing::get(fanbox_image_thumbnail),
        )
        .route(
            "/thumbnail/file/{id}",
            axum::routing::get(fanbox_file_thumbnail),
        )
}
