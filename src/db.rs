use std::collections::HashMap;

use serde::Serialize;
use sqlx::SqlitePool;
use tokio::sync::OnceCell;

use crate::data::{IllustBookmarkTags, IllustState};

static DB: OnceCell<sqlx::SqlitePool> = OnceCell::const_new();

struct TagIterator<I: Iterator<Item = u64> + Clone>(I);
impl<I: Iterator<Item = u64> + Clone> Serialize for TagIterator<I> {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.collect_seq(self.0.clone())
    }
}

async fn get_db() -> anyhow::Result<&'static sqlx::SqlitePool> {
    let db = DB
        .get_or_try_init::<anyhow::Error, _, _>(|| async {
            let key = std::env::var("DATABASE_URL")?;
            let db = SqlitePool::connect(&key).await?;
            Ok(db)
        })
        .await?;

    Ok(db)
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
    illust: &crate::data::Illust,
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

    let update_type = if let Some(orig) = &orig {
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
                create_date=?,
                update_date=?,
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
                ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, datetime('now', 'utc')
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
            "UPDATE illusts SET last_successful_fetch = last_fetch WHERE id = ?",
            illust_id
        )
        .execute(&mut *tx)
        .await?;
    }

    if let Some(detail) = illust.data.as_detail() {
        // Update details
        let row_changed = sqlx::query!(
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
        assert_eq!(row_changed, 1);
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
