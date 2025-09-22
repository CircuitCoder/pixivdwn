use clap::Args;

use crate::data::IllustState;

#[derive(clap::ValueEnum, Clone, Copy)]
pub enum QueryDownloadState {
    /// Only fully downloaded illustrations
    FullyDownloaded,

    /// Only illustrations with missing downloaded pages
    NotFullyDownloaded,
}

#[derive(clap::ValueEnum, Clone, Copy)]
pub enum QueryOrder {
    /// Order by illustration ID, ascending
    IdAsc,

    /// Order by illustration ID, descending
    IdDesc,

    /// Order by bookmark ID, ascending
    BookmarkIdAsc,

    /// Order by bookmark ID, descending
    BookmarkIdDesc,
}

#[derive(clap::ValueEnum, Clone, Copy)]
pub enum Format {
    /// Count only
    Count,

    /// ID only
    ID,

    /// Output in JSON
    JSON,
}

#[derive(Args)]
pub struct Query {
    /// Illustration ID
    #[arg(short, long)]
    id: Option<u64>,

    /// Illustration state
    #[arg(short, long, value_enum)]
    state: Option<IllustState>,

    /// Download state
    #[arg(short, long, value_enum)]
    download_state: Option<QueryDownloadState>,

    /// Tag, can appear multiple times to specify multiple tags (AND)
    #[arg(short, long)]
    tag: Vec<String>,

    /// Bookmark tag, can appear multiple times to specify multiple tags (AND)
    #[arg(short, long)]
    bookmark_tag: Vec<String>,

    /// Ordering
    #[arg(short, long, value_enum, default_value_t = QueryOrder::IdAsc)]
    order: QueryOrder,

    /// Limit
    #[arg(short, long)]
    limit: Option<usize>,

    /// Output format
    #[arg(short, long, value_enum, default_value_t = Format::ID)]
    format: Format,

    /// Print SQL query
    #[arg(long)]
    print_sql: bool,

    /// Dry run
    #[arg(long)]
    dry_run: bool,
}

impl Query {
    pub async fn run(self) -> anyhow::Result<()> {
        // You know what, let's concat SQL
        
        let mut sql = format!("SELECT {} FROM illusts", 
            match self.format {
                Format::Count => "COUNT(*) as count",
                Format::ID => "id",
                Format::JSON => "*",
            }
        );

        let mut wheres = Vec::new();
        if let Some(id) = self.id {
            wheres.push(format!("id = {}", id));
        }

        if let Some(state) = self.state {
            wheres.push(format!("state = {}", state as u8));
        }

        if let Some(download_state) = self.download_state {
            // This is a little more complex. We need to query the downloaded image table
            // to get the number of downloaded pages, and compare with the fetched number of pages.

            wheres.push(format!(
                r#"
                  page_count {} (
                    SELECT COUNT(*) FROM images
                    WHERE illust_id = illusts.id
                  )
                "#,
                match download_state {
                    QueryDownloadState::FullyDownloaded => "=",
                    QueryDownloadState::NotFullyDownloaded => "!=",
                }
            ));
        }

        if self.tag.len() > 0 {
            // Query the tags table, and asserts that not linked tags do not exist
            wheres.push(format!(
                r#"NOT EXISTS (
                  SELECT id FROM tags
                  WHERE tag IN (SELECT json_each.value FROM json_each('{}'))
                  AND id NOT IN (
                    SELECT tag_id FROM illust_tags WHERE illust_id = illusts.id
                  )
                )"#,
                serde_json::to_string(&self.tag)?
            ));

            // Additional constraints that all tags must exists
            wheres.push(format!(
                "(SELECT COUNT(*) FROM tags WHERE tag IN (SELECT json_each.value FROM json_each('{}'))) = {}",
                serde_json::to_string(&self.tag)?,
                self.tag.len()
            ));
        }

        if self.bookmark_tag.len() > 0 {
            // Query the tags table, and asserts that not linked tags do not exist
            wheres.push(format!(
                r#"NOT EXISTS (
                  SELECT id FROM tags
                  WHERE tag IN (SELECT json_each.value FROM json_each('{}'))
                  AND id NOT IN (
                    SELECT tag_id FROM illust_bookmark_tags WHERE illust_id = illusts.id
                  )
                )"#,
                serde_json::to_string(&self.bookmark_tag)?
            ));

            // Additional constraints that all tags must exists
            wheres.push(format!(
                "(SELECT COUNT(*) FROM tags WHERE tag IN (SELECT json_each.value FROM json_each('{}'))) = {}",
                serde_json::to_string(&self.tag)?,
                self.tag.len()
            ));
        }

        if wheres.len() > 0 {
            sql.push_str(" WHERE ");
            sql.push_str(&wheres.join(" AND "));
        }

        sql.push_str(" ORDER BY ");
        match self.order {
            QueryOrder::IdAsc => sql.push_str("id ASC"),
            QueryOrder::IdDesc => sql.push_str("id DESC"),
            QueryOrder::BookmarkIdAsc => sql.push_str("bookmark_id ASC"),
            QueryOrder::BookmarkIdDesc => sql.push_str("bookmark_id DESC"),
        }

        if let Some(limit) = self.limit {
            sql.push_str(&format!(" LIMIT {}", limit));
        }

        if self.print_sql {
            println!("{}", sql);
        }

        if self.dry_run {
            return Ok(());
        }

        let result = crate::db::query_raw(&sql).await?;
        use sqlx::Row;

        match self.format {
            Format::Count => {
                let row = result.into_iter().next().unwrap();
                let count: i64 = row.try_get("count")?;
                println!("{}", count);
            },
            Format::ID => {
                for row in result {
                    let id: u64 = row.try_get("id")?;
                    println!("{}", id);
                }
            },
            Format::JSON => {
                unimplemented!();
            },
        }
        Ok(())
    }
}
