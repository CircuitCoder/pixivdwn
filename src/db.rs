use std::collections::HashMap;

use serde::Serialize;
use sqlx::{
    SqlitePool,
    migrate::{Migrator, Migrate},
    sqlite::{SqliteConnectOptions, SqliteRow},
};
use tokio::sync::OnceCell;

use crate::data::pixiv::{IllustBookmarkTags, IllustState, UgoiraFrame};

static DB: OnceCell<sqlx::SqlitePool> = OnceCell::const_new();
static DBURL: OnceCell<String> = OnceCell::const_new();
static MIGRATOR: Migrator = sqlx::migrate!();

pub async fn set_url(url: String) -> anyhow::Result<()> {
    DBURL
        .set(url)
        .map_err(|_| anyhow::anyhow!("Database URL can only be set once"))
}

struct TagIterator<I: Iterator<Item = u64> + Clone>(I);
impl<I: Iterator<Item = u64> + Clone> Serialize for TagIterator<I> {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.collect_seq(self.0.clone())
    }
}

async fn get_db() -> anyhow::Result<&'static sqlx::SqlitePool> {
    let db = DB
        .get_or_try_init::<anyhow::Error, _, _>(|| async {
            let url = DBURL
                .get()
                .ok_or_else(|| anyhow::anyhow!("Database URL not set"))?;
            let db = SqlitePool::connect(&url).await?;

            let mut conn = db.acquire().await?;
            conn.ensure_migrations_table().await?;
            let applied_migrations: HashMap<_, _> = conn.list_applied_migrations().await?
                .into_iter().map(|e| (e.version, e.checksum)).collect();
            for migration in MIGRATOR.iter() {
                if migration.migration_type.is_down_migration() {
                    continue;
                }
                match applied_migrations.get(&migration.version) {
                    None => return Err(anyhow::anyhow!("Database migration pending, please run `pixivdwn database setup`")),
                    Some(checksum) if checksum != &migration.checksum => {
                        return Err(anyhow::anyhow!("Database migration version {} checksum mismatch, possible corruption", migration.version));
                    }
                    _ => {},
                }
            }

            Ok(db)
        })
        .await?;
    Ok(db)
}

pub async fn setup_db() -> anyhow::Result<()> {
    let db = DB
        .get_or_try_init::<anyhow::Error, _, _>(|| async {
            let url = DBURL
                .get()
                .ok_or_else(|| anyhow::anyhow!("Database URL not set"))?;
            let opts: SqliteConnectOptions = url.parse()?;
            let opts = opts.create_if_missing(true);
            let db = SqlitePool::connect_with(opts).await?;
            Ok(db)
        })
        .await?;
    MIGRATOR.run(db).await?;
    Ok(())
}

pub async fn get_tag_mapping<S: AsRef<str>>(tag: S) -> anyhow::Result<u64> {
    // Upsert tags one by one, guarantees atomicity
    let db = get_db().await?;
    let tag = tag.as_ref();
    let rec = sqlx::query!("INSERT INTO tags (tag) VALUES (?) ON CONFLICT(tag) DO UPDATE SET tag=excluded.tag RETURNING id", tag)
        .fetch_one(db)
        .await?;
    Ok(rec.id as u64)
}

// TODO: detech completely unchanged
#[derive(PartialEq, Eq)]
pub enum IllustUpdateResult {
    Inserted,
    BookmarkIDChanged,
    Updated,
    Skipped,
}

pub async fn update_illust(
    illust: &crate::data::pixiv::Illust,
    tag_map_ctx: &mut HashMap<String, u64>,
) -> anyhow::Result<IllustUpdateResult> {
    // Update illust content (title, caption, etc.)

    // Transaction

    let db = get_db().await?;

    // Before locking the database, upsert all tags
    if let Some(inner) = illust.data.as_simple() {
        for t in inner.tags.tag_names() {
            if tag_map_ctx.contains_key(t) {
                continue;
            }
            let id = get_tag_mapping(t).await?;
            tag_map_ctx.insert(t.to_owned(), id);
        }
    }

    if let Some(inner) = illust.bookmark.as_ref()
        && let IllustBookmarkTags::Known(tags) = &inner.tags
    {
        for t in tags.iter() {
            if tag_map_ctx.contains_key(t.as_str()) {
                continue;
            }
            let id = get_tag_mapping(t).await?;
            tag_map_ctx.insert(t.clone(), id);
        }
    }

    let mut tx = db.begin().await?;

    // Update author first s.t. foreign key is satisfied
    if let Some(inner) = &illust.data.as_simple() {
        let author_id = inner.author.id as i64;
        sqlx::query!(
            r#"
            INSERT INTO authors (id, name)
            VALUES (?, ?)
            ON CONFLICT(id) DO UPDATE SET
                name=excluded.name
            "#,
            author_id,
            inner.author.name,
        )
        .execute(&mut *tx)
        .await?;

        if let Some(account) = &inner.author.account {
            let rows_affected = sqlx::query!(
                r#"
                UPDATE authors SET account = ?
                WHERE id = ?
                "#,
                account,
                author_id,
            )
            .execute(&mut *tx)
            .await?
            .rows_affected();
            assert_eq!(rows_affected, 1);
        }
    }

    let illust_id = illust.id as i64;
    // Upsert main illust row:
    // Insert if not exists
    // If exists and previously masked, insert anyway
    // If exists and previously unlisted, only update if new state is not masked
    // If exists and previously normal, only update if the new state is normal
    // Returns whether the illust was "new", in the sense that it was inserted or the bookmark id changed

    let orig = sqlx::query!(
        r#"SELECT bookmark_id, illust_state as "illust_state: IllustState" FROM illusts WHERE id = ?"#,
        illust_id
    )
        .fetch_optional(&mut *tx)
        .await?;

    let fetched_data = illust.data.as_simple();
    let fetched_title = fetched_data.map(|d| d.title.as_str());
    let fetched_author_id = fetched_data.map(|d| d.author.id as i64);
    let fetched_create_date = fetched_data.map(|d| d.create_date);
    let fetched_update_date = fetched_data.map(|d| d.update_date);
    let fetched_x_restrict = fetched_data.map(|d| d.x_restrict);
    let fetched_ai_type = fetched_data.map(|d| d.ai_type);
    let illust_bookmark_id = illust.bookmark.as_ref().map(|b| b.id as i64);
    let illust_bookmark_private = illust.bookmark.as_ref().map(|b| b.private);
    let fetched_illust_type = fetched_data.map(|d| d.illust_type);
    let fetched_page_count = fetched_data.map(|d| d.page_count as i64);

    let update_type: IllustUpdateResult = if let Some(orig) = &orig {
        // Set last_fetch no matter what
        sqlx::query!(
            "UPDATE illusts SET last_fetch = datetime('now', 'utc') WHERE id = ?",
            illust_id
        )
        .execute(&mut *tx)
        .await?;

        let skip = match (orig.illust_state, illust.state) {
            (IllustState::Masked, _) => false,
            (IllustState::Unlisted, IllustState::Masked) => true,
            (IllustState::Unlisted, _) => false,
            (IllustState::Normal, IllustState::Masked) => true,
            (IllustState::Normal, IllustState::Unlisted) => true,
            (IllustState::Normal, IllustState::Normal) => false,
        };
        if skip {
            return Ok(IllustUpdateResult::Skipped);
        }

        // Do update
        sqlx::query!(
            r#"UPDATE illusts SET
                title=?,
                author_id=?,
                create_date=datetime(?, 'utc'),
                update_date=datetime(?, 'utc'),
                x_restrict=?,
                ai_type=?,
                illust_state=?,
                bookmark_id=?,
                bookmark_private=?,
                illust_type=?,
                page_count=?
            WHERE id = ?"#,
            fetched_title,
            fetched_author_id,
            fetched_create_date,
            fetched_update_date,
            fetched_x_restrict,
            fetched_ai_type,
            illust.state,
            illust_bookmark_id,
            illust_bookmark_private,
            fetched_illust_type,
            fetched_page_count,
            illust_id,
        )
        .execute(&mut *tx)
        .await?;

        if orig.bookmark_id != illust_bookmark_id {
            IllustUpdateResult::BookmarkIDChanged
        } else {
            IllustUpdateResult::Updated
        }
    } else {
        // No previous line, insert
        sqlx::query!(
            r#"INSERT INTO illusts (
                id,
                title,
                author_id,
                create_date,
                update_date,
                x_restrict,
                ai_type,
                illust_state,
                bookmark_id,
                bookmark_private,
                illust_type,
                page_count,
                last_fetch
            ) VALUES (
                ?, ?, ?, datetime(?, 'utc'), datetime(?, 'utc'), ?, ?, ?, ?, ?, ?, ?, datetime('now', 'utc')
            )"#,
            illust_id,
            fetched_title,
            fetched_author_id,
            fetched_create_date,
            fetched_update_date,
            fetched_x_restrict,
            fetched_ai_type,
            illust.state,
            illust_bookmark_id,
            illust_bookmark_private,
            fetched_illust_type,
            fetched_page_count,
        ).execute(&mut *tx)
            .await?;
        IllustUpdateResult::Inserted
    };

    // If this is normal, also update last_successful_fetch, copy from last_fetch
    if let IllustState::Normal = illust.state {
        sqlx::query!(
            "UPDATE illusts SET last_successful_fetch = last_fetch, corrupted = FALSE WHERE id = ?",
            illust_id
        )
        .execute(&mut *tx)
        .await?;
    }

    if let Some(detail) = illust.data.as_detail() {
        // Update details
        let rows_affected = sqlx::query!(
            r#"UPDATE illusts SET
                content_desc=?,
                content_is_howto=?,
                content_is_original=?,
                last_successful_content_fetch = last_fetch
            WHERE id = ?"#,
            detail.desc,
            detail.is_howto,
            detail.is_original,
            illust_id,
        )
        .execute(&mut *tx)
        .await?
        .rows_affected();
        assert_eq!(rows_affected, 1);
    }

    // TODO: add tag details
    if let Some(inner) = illust.data.as_simple() {
        let tags: Vec<_> = inner.tags.tag_names().map(str::to_owned).collect();
        let tags_iterator = tags.iter().map(|t| *tag_map_ctx.get(t).unwrap());
        tag_illust(&mut tx, illust.id, tags_iterator).await?;
    }

    // Update bookmark tags
    if let Some(inner) = illust.bookmark.as_ref()
        && let IllustBookmarkTags::Known(tags) = &inner.tags
    {
        let bookmark_tags_iterator = tags.iter().map(|t| *tag_map_ctx.get(t.as_str()).unwrap());
        tag_illust_bookmark(&mut tx, illust.id, bookmark_tags_iterator).await?;
    }

    tx.commit().await?;

    Ok(update_type)
}

async fn tag_illust(
    tx: &mut sqlx::Transaction<'static, sqlx::Sqlite>,
    illust_id: u64,
    tags: impl Iterator<Item = u64> + Clone,
) -> anyhow::Result<()> {
    // Insert new tags
    let illust_id = illust_id as i64;
    let tags_str = serde_json::to_string(&TagIterator(tags.clone()))?;
    for tag in tags {
        let tag = tag as i64;
        sqlx::query!(
            "INSERT OR IGNORE INTO illust_tags (illust_id, tag_id) VALUES (?, ?)",
            illust_id,
            tag
        )
        .execute(&mut **tx)
        .await?;
    }
    // Delete tags that are not in the new set
    sqlx::query!("DELETE FROM illust_tags WHERE illust_id = ? AND tag_id NOT IN (SELECT json_each.value FROM json_each(?))", illust_id, tags_str)
        .execute(&mut **tx)
        .await?;
    Ok(())
}

async fn tag_illust_bookmark(
    tx: &mut sqlx::Transaction<'static, sqlx::Sqlite>,
    illust_id: u64,
    tags: impl Iterator<Item = u64> + Clone,
) -> anyhow::Result<()> {
    // Insert new tags
    let illust_id = illust_id as i64;
    let tags_str = serde_json::to_string(&TagIterator(tags.clone()))?;
    for tag in tags {
        let tag = tag as i64;
        sqlx::query!(
            "INSERT OR IGNORE INTO illust_bookmark_tags (illust_id, tag_id) VALUES (?, ?)",
            illust_id,
            tag
        )
        .execute(&mut **tx)
        .await?;
    }
    // Delete tags that are not in the new set
    sqlx::query!("DELETE FROM illust_bookmark_tags WHERE illust_id = ? AND tag_id NOT IN (SELECT json_each.value FROM json_each(?))", illust_id, tags_str)
        .execute(&mut **tx)
        .await?;
    Ok(())
}

pub async fn update_image(
    illust: u64,
    page: usize,
    url: &str,
    path: &str,
    width: u64,
    height: u64,
    ugoira_frames: Option<Vec<UgoiraFrame>>,
) -> anyhow::Result<()> {
    let db = get_db().await.unwrap();
    let illust = illust as i64;
    let page = page as i64;
    let width = width as i64;
    let height = height as i64;
    let ugoira_frames = ugoira_frames
        .map(|f| serde_json::to_string(&f))
        .transpose()?;

    sqlx::query!(
        r#"INSERT INTO images (illust_id, page, url, path, download_date, width, height, ugoira_frames)
        VALUES (?, ?, ?, ?, datetime('now', 'utc'), ?, ?, ?)
        ON CONFLICT(illust_id, page) DO UPDATE SET
            url=excluded.url,
            path=excluded.path,
            download_date=excluded.download_date,
            width=excluded.width,
            height=excluded.height,
            ugoira_frames=excluded.ugoira_frames
        "#,
        illust,
        page,
        url,
        path,
        width,
        height,
        ugoira_frames,
    )
    .execute(db)
    .await?;

    Ok(())
}

pub async fn get_illust_type(
    illust_id: u64,
) -> anyhow::Result<Option<crate::data::pixiv::IllustType>> {
    let db = get_db().await?;
    let illust_id = illust_id as i64;
    let rec = sqlx::query!(
        r#"SELECT illust_type as "illust_type!: crate::data::pixiv::IllustType" FROM illusts WHERE id = ?"#,
        illust_id,
    )
    .fetch_optional(db)
    .await?;
    Ok(rec.map(|r| r.illust_type))
}

pub async fn get_existing_pages<B: FromIterator<usize>>(illust_id: u64) -> anyhow::Result<B> {
    let db = get_db().await?;
    let illust_id = illust_id as i64;
    let recs = sqlx::query!(
        r#"SELECT page FROM images WHERE illust_id = ? ORDER BY page ASC"#,
        illust_id,
    )
    .fetch_all(db)
    .await?;
    Ok(recs.into_iter().map(|r| r.page as usize).collect())
}

pub async fn query_raw(sql: &str) -> anyhow::Result<Vec<SqliteRow>> {
    let db = get_db().await?;
    let result = sqlx::query(sql).fetch_all(db).await?;
    Ok(result)
}

#[derive(PartialEq, Eq)]
pub enum FanboxPostUpdateResult {
    Inserted,
    Updated,
    Skipped,
}

pub async fn update_fanbox_post(
    detail: &crate::data::fanbox::FetchPostDetail,
) -> anyhow::Result<FanboxPostUpdateResult> {
    let post = &detail.post;

    let db = get_db().await?;
    let post_id = post.id as i64;
    let creator_id = &post.creator_id;
    let title = &post.title;
    let body = detail.body.text_repr()?;
    let is_body_rich = detail.body.is_rich();
    let fee = post.fee_required as i64;
    let published_datetime = post.published_datetime;
    let updated_datetime = post.updated_datetime;
    let is_adult = post.has_adult_content;

    let orig = sqlx::query!(r#"SELECT id, updated_datetime as "updated_datetime: chrono::DateTime<chrono::Utc>" FROM fanbox_posts WHERE id = ?"#, post_id)
        .fetch_optional(db)
        .await?;

    if let Some(orig) = orig {
        if orig.updated_datetime == updated_datetime {
            return Ok(FanboxPostUpdateResult::Skipped);
        } else if orig.updated_datetime > updated_datetime {
            tracing::warn!(
                "Post {} updated_datetime went backwards: was {}, now {}",
                post_id,
                orig.updated_datetime,
                updated_datetime
            );
            return Ok(FanboxPostUpdateResult::Skipped);
        }

        sqlx::query!(
            r#"UPDATE fanbox_posts SET
                creator_id=?,
                title=?,
                body=?,
                is_body_rich=?,
                fee=?,
                published_datetime=datetime(?, 'utc'),
                updated_datetime=datetime(?, 'utc'),
                is_adult=?,
                fetched_at=datetime('now', 'utc')
            WHERE id = ?"#,
            creator_id,
            title,
            body,
            is_body_rich,
            fee,
            published_datetime,
            updated_datetime,
            is_adult,
            post_id,
        )
        .execute(db)
        .await?;
        Ok(FanboxPostUpdateResult::Updated)
    } else {
        sqlx::query!(
            r#"INSERT INTO fanbox_posts (
                id,
                creator_id,
                title,
                body,
                is_body_rich,
                fee,
                published_datetime,
                updated_datetime,
                is_adult,
                fetched_at
            ) VALUES (
                ?, ?, ?, ?, ?, ?, datetime(?, 'utc'), datetime(?, 'utc'), ?, datetime('now', 'utc')
            )"#,
            post_id,
            creator_id,
            title,
            body,
            is_body_rich,
            fee,
            published_datetime,
            updated_datetime,
            is_adult,
        )
        .execute(db)
        .await?;
        Ok(FanboxPostUpdateResult::Inserted)
    }
}

pub async fn query_fanbox_post_updated_datetime(
    post_id: u64,
) -> anyhow::Result<Option<chrono::DateTime<chrono::Utc>>> {
    let db = get_db().await?;
    let post_id = post_id as i64;
    let rec = sqlx::query!(r#"SELECT updated_datetime as "updated_datetime: chrono::DateTime<chrono::Utc>" FROM fanbox_posts WHERE id = ?"#, post_id)
        .fetch_optional(db)
        .await?;
    Ok(rec.map(|r| r.updated_datetime))
}

pub async fn add_fanbox_image(
    post_id: u64,
    idx: usize,
    img: &crate::data::fanbox::FetchPostImage,
) -> anyhow::Result<bool> {
    let db = get_db().await?;
    let id = &img.id;
    let post_id = post_id as i64;
    let url = &img.original_url;
    let width = img.width as i64;
    let height = img.height as i64;
    let ext = &img.extension;
    let idx = idx as i64;

    let ret = sqlx::query!(
        r#"INSERT OR IGNORE INTO fanbox_images (
            id,
            post_id,
            url,
            width,
            height,
            ext,
            idx
        ) VALUES (
            ?, ?, ?, ?, ?, ?, ?
        )"#,
        id,
        post_id,
        url,
        width,
        height,
        ext,
        idx,
    )
    .execute(db)
    .await?
    .rows_affected();

    Ok(ret > 0)
}

pub async fn add_fanbox_file(
    post_id: u64,
    idx: usize,
    file: &crate::data::fanbox::FetchPostFile,
) -> anyhow::Result<bool> {
    let db = get_db().await?;

    let id = &file.id;
    let post_id = post_id as i64;
    let name = &file.name;
    let url = &file.url;
    let size = file.size as i64;
    let ext = &file.extension;
    let idx = idx as i64;

    let ret = sqlx::query!(
        r#"INSERT OR IGNORE INTO fanbox_files (
            id,
            post_id,
            name,
            url,
            size,
            ext,
            idx
        ) VALUES (
            ?, ?, ?, ?, ?, ?, ?
        )"#,
        id,
        post_id,
        name,
        url,
        size,
        ext,
        idx
    )
    .execute(db)
    .await?
    .rows_affected();

    Ok(ret > 0)
}

pub struct FanboxFileDownloadSpec {
    pub url: String,
    pub name: String,
    pub post_id: String,
    pub ext: String,
    pub idx: i64,
}

pub async fn query_fanbox_file_dwn(id: &str) -> anyhow::Result<Option<FanboxFileDownloadSpec>> {
    let db = get_db().await?;
    let rec = sqlx::query_as!(
        FanboxFileDownloadSpec,
        "SELECT url, name, post_id, ext, idx FROM fanbox_files WHERE id = ?",
        id
    )
    .fetch_optional(db)
    .await?;
    Ok(rec)
}

pub struct FanboxImageDownloadSpec {
    pub url: String,
    pub post_id: String,
    pub ext: String,
    pub idx: i64,
}

pub async fn query_fanbox_image_dwn(id: &str) -> anyhow::Result<Option<FanboxImageDownloadSpec>> {
    let db = get_db().await?;
    let rec = sqlx::query_as!(
        FanboxImageDownloadSpec,
        "SELECT url, post_id, ext, idx FROM fanbox_images WHERE id = ?",
        id
    )
    .fetch_optional(db)
    .await?;
    Ok(rec)
}

pub async fn update_file_download(id: &str, path: &str) -> anyhow::Result<bool> {
    let db = get_db().await?;
    let rows_updated = sqlx::query!(
        "UPDATE fanbox_files SET path = ?, downloaded_at = datetime('now', 'utc') WHERE id = ?",
        path,
        id
    )
    .execute(db)
    .await?
    .rows_affected();
    Ok(rows_updated > 0)
}

pub async fn update_image_download(id: &str, path: &str) -> anyhow::Result<bool> {
    let db = get_db().await?;
    let rows_updated = sqlx::query!(
        "UPDATE fanbox_images SET path = ?, downloaded_at = datetime('now', 'utc') WHERE id = ?",
        path,
        id
    )
    .execute(db)
    .await?
    .rows_affected();
    Ok(rows_updated > 0)
}
