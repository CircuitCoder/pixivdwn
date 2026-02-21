use clap::{Args, Subcommand};
use futures::StreamExt;

use crate::{
    data::fanbox::FanboxRequest,
    util::{DatabasePathFormat, DownloadIdSrc, DownloadResult, TerminationCondition},
};

#[derive(Args)]
pub struct Fanbox {
    #[command(subcommand)]
    cmd: FanboxCmd,
}

#[derive(clap::ValueEnum, Clone, Copy)]
pub enum FanboxAttachmentType {
    File,
    Image,
}

#[derive(Args)]
#[group(required = false, multiple = false)]
pub struct FanboxSyncSrc {
    /// ID of the Fanbox creator. If given, sync all posts from this creator.
    creator: Option<String>,

    /// ID of a specific post. If given, only sync this one post.
    #[arg(short, long)]
    post: Option<u64>,
}

#[derive(Args)]
pub struct FanboxSyncArgs {
    #[command(flatten)]
    src: FanboxSyncSrc,

    /// Termination condition (alias: --term)
    #[arg(alias="term", long, value_enum, default_value_t = TerminationCondition::UntilEnd)]
    termination: TerminationCondition,

    /// Skip pages. Can only be used when `creator` is specified
    #[arg(long, requires("creator"))]
    skip_pages: Option<usize>,

    /// Skip failed posts instead of aborting
    #[arg(long, default_value_t = false)]
    skip_failed: bool,

    /// Maximum number of retries for post fetch
    #[arg(short, long, default_value_t = 0)]
    retries: usize,

    /// Exponential backoff base for retries
    #[arg(long)]
    retry_backoff: Option<usize>,
}

impl FanboxSyncArgs {
    async fn sync_post(
        &self,
        session: &crate::config::Session,
        db: &crate::db::Database,
        id: u64,
    ) -> anyhow::Result<()> {
        let mut tries = 0;
        let mut backoff = self.retry_backoff;
        let mut detail = loop {
            match crate::data::fanbox::fetch_post(session, id).await {
                Err(e) => {
                    tracing::warn!("Failed to fetch post {}: {}", id, e);
                    if tries == self.retries {
                        tracing::error!(
                            "Failed to fetch post {} after {} tries",
                            id,
                            1 + self.retries
                        );

                        return Err(e);
                    }
                    tries += 1;
                    if let Some(ref mut b) = backoff {
                        tracing::info!("Backing off for {}ms", *b);
                        tokio::time::sleep(std::time::Duration::from_millis(*b as u64)).await;
                        *b *= 2;
                    }
                }
                Ok(d) => break d,
            }
        };

        let updated = db.update_fanbox_post(&detail).await?;
        let prompt = match updated {
            crate::db::FanboxPostUpdateResult::Inserted => "Inserted",
            crate::db::FanboxPostUpdateResult::Updated => "Updated",
            crate::db::FanboxPostUpdateResult::Skipped => "Skipped",
        };

        tracing::info!("{} post {} - {}", prompt, id, detail.post.title);

        if let Some(ref mut body) = detail.body {
            for (idx, file) in body.files() {
                let added = db.add_fanbox_file(detail.post.id, idx, file).await?;
                if added {
                    tracing::info!("  Added {}: file {} - {}", idx, file.id, file.name);
                }
            }

            for (idx, image) in body.images() {
                let added = db.add_fanbox_image(detail.post.id, idx, image).await?;
                if added {
                    tracing::info!("  Added {}: image {}", idx, image.id);
                }
            }
        }

        Ok(())
    }

    async fn sync_creator(
        &self,
        session: &crate::config::Session,
        db: &crate::db::Database,
        creator: &str,
    ) -> anyhow::Result<()> {
        let mut posts = Box::pin(crate::data::fanbox::fetch_author_posts(
            session,
            creator,
            self.skip_pages.unwrap_or(0),
        ));

        while let Some(post) = posts.next().await.transpose()? {
            let orig = db.query_fanbox_post_status(post.id).await?;
            if let Some(orig) = orig
                && !orig.needs_update(&post)
            {
                tracing::info!(
                    "Post {} not updated since last fetch at {}, skipping",
                    post.id,
                    orig.updated_datetime
                );

                if matches!(self.termination, TerminationCondition::OnHit) {
                    tracing::info!("Encountered an already existing post. Terminating.");
                    break;
                } else {
                    continue;
                }
            }

            let ret = self.sync_post(session, db, post.id).await;
            if !self.skip_failed && ret.is_err() {
                return ret;
            }
        }
        Ok(())
    }

    async fn sync_all(
        &self,
        session: &crate::config::Session,
        db: &crate::db::Database,
    ) -> anyhow::Result<()> {
        let creators = crate::data::fanbox::fetch_supporting_list(session).await?;
        for creator in creators {
            tracing::info!(
                "Syncing creator {} ({})",
                creator
                    .user
                    .as_ref()
                    .map(|e| e.name.as_str())
                    .unwrap_or("?"),
                creator.creator_id
            );
            self.sync_creator(session, db, &creator.creator_id).await?;
        }
        Ok(())
    }

    pub async fn run(
        &self,
        session: &crate::config::Session,
        db: &crate::db::Database,
    ) -> anyhow::Result<()> {
        if let Some(p) = self.src.post {
            self.sync_post(session, db, p).await
        } else if let Some(ref c) = self.src.creator {
            self.sync_creator(session, db, c).await
        } else {
            self.sync_all(session, db).await
        }
    }
}

#[derive(Args)]
pub struct FanboxDownloadArgs {
    /// Type of the downloaded item
    #[arg(value_enum)]
    r#type: FanboxAttachmentType,

    #[clap(flatten)]
    /// ID of the image / file
    id: DownloadIdSrc<String>,

    /// Abort if failed
    #[arg(long)]
    abort_on_fail: bool,

    /// Create base directory if not exist
    #[arg(long)]
    mkdir: bool,

    /// Canonicalization for paths recorded in database
    #[arg(long, value_enum, default_value_t = DatabasePathFormat::Absolute)]
    database_path_format: DatabasePathFormat,

    /// Show progress bar. The download speed is based on the *UNZIPPED* stream, so don't be surprised if it exceeds your bandwidth.
    #[arg(short, long)]
    progress: bool,
}

impl FanboxDownloadArgs {
    async fn download_single(
        &self,
        session: &crate::config::Session,
        db: &crate::db::Database,
        id: &str,
    ) -> anyhow::Result<()> {
        let (url, filename) = get_download_spec(db, self.r#type, id).await?;
        let DownloadResult {
            written_path,
            final_path,
            size,
        } = crate::util::download_then_persist(
            FanboxRequest(session),
            session.get_fanbox_base_dir()?,
            &filename,
            self.database_path_format,
            &url,
            self.progress,
        )
        .await?;
        let updated = match self.r#type {
            FanboxAttachmentType::Image => {
                let (width, height) = crate::util::get_image_dim(
                    std::fs::File::open(&final_path)?,
                    &final_path,
                    None,
                )?;
                db.update_fanbox_image_download(
                    &id,
                    written_path.to_str().unwrap(),
                    width as i64,
                    height as i64,
                )
                .await?
            }
            FanboxAttachmentType::File => {
                db.update_fanbox_file_download(&id, written_path.to_str().unwrap(), size as i64)
                    .await?
            }
        };

        assert!(
            updated,
            "{} {} should exist in database. Possible DB race",
            match self.r#type {
                FanboxAttachmentType::File => "File",
                FanboxAttachmentType::Image => "Image",
            },
            id
        );

        Ok(())
    }

    pub async fn run(
        self,
        session: &crate::config::Session,
        db: &crate::db::Database,
    ) -> anyhow::Result<()> {
        if self.mkdir {
            tokio::fs::create_dir_all(session.get_fanbox_base_dir()?).await?;
        }

        let mut collected_errs = Vec::new();
        for id in self.id.read()? {
            let id = id?;
            if let Err(e) = self.download_single(session, db, &id).await {
                if self.abort_on_fail {
                    return Err(e);
                } else {
                    tracing::error!("Failed to download {}: {:?}", id, e);
                    collected_errs.push((id, e));
                }
            };
        }

        if collected_errs.is_empty() {
            Ok(())
        } else {
            // TODO: use thiserror
            Err(anyhow::anyhow!(
                "{} errors occurred during download",
                collected_errs.len()
            ))
        }
    }
}

#[derive(clap::ValueEnum, Clone, Copy)]
pub enum QueryOrder {
    /// Order by (post_id ASC, idx ASC)
    PostAsc,

    /// Order by (post_id DESC, idx ASC)
    PostDesc,
}

#[derive(Args)]
pub struct FanboxAttachmentArgs {
    /// Type of the queried item
    #[arg(value_enum)]
    r#type: FanboxAttachmentType,

    /// ID of some specific image
    id: Option<String>,

    /// ID of the related post
    #[arg(short, long)]
    post: Option<u64>,

    /// Specify the download state
    #[arg(short, long)]
    downloaded: Option<bool>,

    /// Ordering of the returned ids
    #[arg(short, long, value_enum, default_value_t = QueryOrder::PostDesc)]
    order: QueryOrder,

    /// Print SQL query
    #[arg(long)]
    print_sql: bool,

    /// Dry-run
    #[arg(long)]
    dry_run: bool,
}

impl FanboxAttachmentArgs {
    pub async fn run(
        &self,
        _session: &crate::config::Session,
        db: &crate::db::Database,
    ) -> anyhow::Result<()> {
        // Just like query, we do SQL concat
        let tbl = match self.r#type {
            FanboxAttachmentType::File => "fanbox_files",
            FanboxAttachmentType::Image => "fanbox_images",
        };

        // TODO: output format
        let mut sql = format!("SELECT id from {}", tbl);
        let mut wheres = Vec::new();

        if let Some(ref id) = self.id {
            wheres.push(format!("id = '{}'", id));
        }
        if let Some(post) = self.post {
            wheres.push(format!("post_id = {}", post));
        }
        if let Some(downloaded) = self.downloaded {
            let predicate = if downloaded {
                "path IS NOT NULL"
            } else {
                "path IS NULL"
            };
            wheres.push(predicate.to_owned())
        }

        if !wheres.is_empty() {
            sql.push_str(" WHERE ");
            sql.push_str(&wheres.join(" AND "));
        }

        match self.order {
            QueryOrder::PostAsc => sql.push_str(" ORDER BY post_id ASC, idx ASC"),
            QueryOrder::PostDesc => sql.push_str(" ORDER BY post_id DESC, idx ASC"),
        }

        if self.print_sql {
            println!("{}", sql);
        }

        if self.dry_run {
            return Ok(());
        }

        let result = db.query_raw(&sql).await?;
        use sqlx::Row;

        for row in result {
            let id: &str = row.try_get("id")?;
            println!("{}", id);
        }

        Ok(())
    }
}

#[derive(Subcommand)]
pub enum FanboxCmd {
    /// Synchronize a specific post, posts from Fanbox creator, or all supported creators
    Sync(FanboxSyncArgs),

    /// Download a specific synced file or image
    Download(FanboxDownloadArgs),

    /// Attachment query
    Attachment(FanboxAttachmentArgs),
}

impl Fanbox {
    pub async fn run(
        self,
        session: &crate::config::Session,
        db: &crate::db::Database,
    ) -> anyhow::Result<()> {
        match self.cmd {
            FanboxCmd::Sync(sync) => sync.run(session, db).await?,
            FanboxCmd::Download(dwn) => dwn.run(session, db).await?,
            FanboxCmd::Attachment(file) => file.run(session, db).await?,
        }
        Ok(())
    }
}

/// Return (url, filename)
pub async fn get_download_spec(
    db: &crate::db::Database,
    ty: FanboxAttachmentType,
    id: &str,
) -> anyhow::Result<(String, String)> {
    match ty {
        FanboxAttachmentType::File => {
            let spec = db
                .query_fanbox_file_download_spec(id)
                .await?
                .ok_or_else(|| anyhow::anyhow!("File {} not found in database", id))?;
            let filename: String = format!(
                "{}_{}_{}_{}.{}",
                spec.post_id, spec.idx, id, spec.name, spec.ext
            );
            Ok((spec.url, filename))
        }
        FanboxAttachmentType::Image => {
            let spec = db
                .query_fanbox_image_download_spec(id)
                .await?
                .ok_or_else(|| anyhow::anyhow!("Image {} not found in database", id))?;
            let filename = format!("{}_{}_{}.{}", spec.post_id, spec.idx, id, spec.ext);
            Ok((spec.url, filename))
        }
    }
}
