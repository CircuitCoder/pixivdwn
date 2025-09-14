use sqlx::SqlitePool;
use tokio::sync::OnceCell;

static DB: OnceCell<sqlx::SqlitePool> = OnceCell::const_new();

async fn get_db() -> anyhow::Result<&'static sqlx::SqlitePool> {
    let db = DB.get_or_try_init::<anyhow::Error, _, _>(|| async {
        let key = std::env::var("DATABASE_URL")?;
        let db = SqlitePool::connect(&key).await?;
        Ok(db)
    }).await?;

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
