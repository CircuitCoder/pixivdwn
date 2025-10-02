use std::collections::HashSet;

use clap::Args;

use crate::{
    data::pixiv::{IllustType, PixivRequest},
    util::{DatabasePathFormat, DownloadIdSrc, DownloadResult},
};

#[derive(clap::ValueEnum, Clone, Copy)]
enum DownloadType {
    /// Force download as ugoira
    Ugoira,

    /// Force download as image (or manga)
    Image,
}

#[derive(Args)]
pub struct Download {
    #[clap(flatten)]
    /// ID of the illustration
    pub id: DownloadIdSrc<u64>,

    /// Dry run, only fech and print the info
    #[arg(long)]
    dry_run: bool,

    /// Create base directory if not exist
    #[arg(long)]
    mkdir: bool,

    /// Base directory to save the illustration
    ///
    /// The illustrations will be saved as `<base_dir>/<illust_id>_p<page>.<ext>`
    #[arg(short, long, default_value = "images")]
    base_dir: String,

    /// Canonicalization for paths recorded in database
    #[arg(long, value_enum, default_value_t = DatabasePathFormat::Absolute)]
    database_path_format: DatabasePathFormat,

    /// Force download type.
    #[arg(short = 't', long)]
    download_type: Option<DownloadType>,

    /// Show progress bar. The download speed is based on the *UNZIPPED* stream, so don't be surprised if it exceeds your bandwidth.
    #[arg(short, long)]
    progress: bool,

    /// Force downloading existing pages
    #[arg(long)]
    force_redownload: bool,
}

impl Download {
    pub async fn run(self, session: &crate::config::Session) -> anyhow::Result<()> {
        for id in self.id.read()? {
            self.single(id?, session).await?;
        }
        Ok(())
    }

    async fn single(&self, id: u64, session: &crate::config::Session) -> anyhow::Result<()> {
        if self.mkdir {
            std::fs::create_dir_all(&self.base_dir)?;
        }

        let illust_type = crate::db::get_illust_type(id).await?.ok_or_else(|| {
            anyhow::anyhow!(
                "{} not found in DB. Please run `pixivdwn illust {}` first.",
                id,
                id
            )
        })?;
        let induced_download_type = match illust_type {
            IllustType::Ugoira => DownloadType::Ugoira,
            _ => DownloadType::Image,
        };
        let download_type = self.download_type.unwrap_or(induced_download_type);

        let skipped_pages = if self.force_redownload {
            HashSet::new()
        } else {
            crate::db::get_existing_pages(id).await?
        };

        match download_type {
            DownloadType::Image => {
                let pages = crate::data::pixiv::get_illust_pages(session, id).await?;
                let tot_pages = pages.len();
                tracing::info!("Downloading {} pages...", tot_pages);
                for (idx, page) in pages.iter().enumerate() {
                    if skipped_pages.contains(&idx) {
                        tracing::info!("Page {}/{}: Skipping", idx + 1, tot_pages);
                        continue;
                    }

                    let url = &page.urls.original;
                    let filename = url.split('/').last().unwrap();
                    tracing::info!(
                        "Page {}/{}: {} x {}, {} from {}",
                        idx + 1,
                        tot_pages,
                        page.width,
                        page.height,
                        filename,
                        url
                    );
                    assert!(
                        filename.starts_with(format!("{}_p{}.", id, idx).as_str())
                            || filename.starts_with(format!("{}_ugoira{}.", id, idx).as_str())
                    );

                    if !self.dry_run {
                        let DownloadResult { written_path, .. } =
                            self.download_file(session, url, filename).await?;
                        let written_path = written_path
                            .to_str()
                            .ok_or_else(|| anyhow::anyhow!("Failed to convert path"))?;
                        crate::db::update_image(
                            id,
                            idx,
                            url,
                            written_path,
                            page.width,
                            page.height,
                            None,
                        )
                        .await?;
                    }
                }
            }
            DownloadType::Ugoira => {
                if skipped_pages.contains(&0) {
                    tracing::info!("Ugoira already downloaded, skipping");
                    return Ok(());
                }

                let meta = crate::data::pixiv::get_illust_ugoira_meta(session, id).await?;
                tracing::info!("Downloading ugoira...");
                let url = &meta.original_src;
                let filename = url.split('/').last().unwrap();
                tracing::info!("Ugoira pack {} from {}", filename, url);
                assert!(
                    filename.starts_with(format!("{}_ugoira", id).as_str())
                        && filename.ends_with(".zip")
                );

                if !self.dry_run {
                    let DownloadResult {
                        written_path,
                        final_path,
                        ..
                    } = self.download_file(session, url, filename).await?;
                    let mut archive = zip::ZipArchive::new(std::fs::File::open(&final_path)?)?;
                    let mut file = archive.by_name(&meta.frames[0].file)?;
                    let (width, height) = crate::util::get_image_dim(
                        &mut file,
                        &meta.frames[0].file,
                        Some(&meta.mime_type),
                    )?;

                    let written_path = written_path
                        .to_str()
                        .ok_or_else(|| anyhow::anyhow!("Failed to convert path"))?;
                    crate::db::update_image(
                        id,
                        0,
                        url,
                        written_path,
                        width as u64,
                        height as u64,
                        Some(meta.frames),
                    )
                    .await?;
                }
            }
        }

        Ok(())
    }

    async fn download_file(
        &self,
        session: &crate::config::Session,
        url: &str,
        filename: &str,
    ) -> anyhow::Result<DownloadResult> {
        crate::util::download_then_persist(
            PixivRequest(session),
            &self.base_dir,
            filename,
            self.database_path_format,
            url,
            self.progress,
        )
        .await
    }
}
