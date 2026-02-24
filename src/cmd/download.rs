use std::{collections::HashMap, path::Path};

use clap::Args;

use crate::{
    data::pixiv::{IllustType, Page, PixivRequest},
    util::{DatabasePathFormat, DownloadIdSrc, DownloadOverwriteBehavior, DownloadResult},
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
    id: DownloadIdSrc<u64>,

    /// Abort if failed
    #[arg(long)]
    abort_on_fail: bool,

    /// Dry run, only fetch and print the info
    #[arg(long)]
    dry_run: bool,

    /// Create base directory if not exist
    #[arg(long)]
    mkdir: bool,

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
    #[arg(long, value_enum, default_value_t = OnExistingBehavior::Verify)]
    on_existing: OnExistingBehavior,
}

#[derive(clap::ValueEnum, Clone, Copy, Eq, PartialEq)]
pub enum OnExistingBehavior {
    /// Skip pages that are already downloaded
    Skip,

    /// Re-verify existing pages to see if the new one is identical
    Verify,

    /// Forcefully overwrite
    Overwrite,
}

impl Download {
    pub async fn run(
        self,
        session: &crate::config::Session,
        db: &crate::db::Database,
    ) -> anyhow::Result<()> {
        let mut collected_errs = Vec::new();
        for id in self.id.read()? {
            let id = id?;
            if let Err(e) = self.single(id, session, db).await {
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

    async fn single(
        &self,
        id: u64,
        session: &crate::config::Session,
        db: &crate::db::Database,
    ) -> anyhow::Result<()> {
        if self.mkdir {
            std::fs::create_dir_all(session.get_pixiv_base_dir()?)?;
        }

        let illust_type = db.get_illust_type(id).await?.ok_or_else(|| {
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

        let existing: HashMap<usize, String> = db.get_image_existing_for(id).await?.collect();

        enum DownloadSource {
            Page(Page),
            UgoiraMeta(crate::data::pixiv::UgoiraMeta),
        }

        impl DownloadSource {
            fn get_url(&self) -> &str {
                match self {
                    DownloadSource::Page(page) => &page.urls.original,
                    DownloadSource::UgoiraMeta(meta) => &meta.original_src,
                }
            }

            fn get_dimension(&self, path: impl AsRef<Path>) -> anyhow::Result<(u64, u64)> {
                match self {
                    DownloadSource::Page(page) => Ok((page.width, page.height)),
                    DownloadSource::UgoiraMeta(meta) => {
                        let mut archive = zip::ZipArchive::new(std::fs::File::open(path)?)?;
                        let mut file = archive.by_name(&meta.frames[0].file)?;
                        let (width, height) = crate::util::get_image_dim(
                            &mut file,
                            &meta.frames[0].file,
                            Some(&meta.mime_type),
                        )?;
                        Ok((width as u64, height as u64))
                    }
                }
            }

            fn ugoira_frames(&self) -> Option<&Vec<crate::data::pixiv::UgoiraFrame>> {
                match self {
                    DownloadSource::Page(_) => None,
                    DownloadSource::UgoiraMeta(meta) => Some(&meta.frames),
                }
            }
        }

        let sources: Box<dyn ExactSizeIterator<Item = DownloadSource>> = match download_type {
            DownloadType::Image => {
                let pages = crate::data::pixiv::get_illust_pages(session, id).await?;
                Box::new(pages.into_iter().map(DownloadSource::Page))
            }
            DownloadType::Ugoira => {
                let meta = crate::data::pixiv::get_illust_ugoira_meta(session, id).await?;
                Box::new(std::iter::once(DownloadSource::UgoiraMeta(meta)))
            }
        };

        let tot_len = sources.len();
        tracing::info!("Downloading {} sources...", tot_len);

        for (idx, src) in sources.enumerate() {
            if self.on_existing == OnExistingBehavior::Skip && existing.contains_key(&idx) {
                tracing::info!("Source {}/{}: Skipping", idx + 1, tot_len);
                continue;
            }

            let url = src.get_url();
            let filename = url.split('/').last().unwrap();

            match src {
                DownloadSource::Page(ref page) => {
                    tracing::info!(
                        "Illust page {}/{}: {} x {}, {} from {}",
                        idx + 1,
                        tot_len,
                        page.width,
                        page.height,
                        filename,
                        url
                    );
                    assert!(filename.starts_with(format!("{}_p{}.", id, idx).as_str()));
                }
                DownloadSource::UgoiraMeta(ref meta) => {
                    tracing::info!("Ugoira pack {}: {} from {}", filename, meta.mime_type, url);
                    assert!(
                        filename.starts_with(format!("{}_ugoira", id).as_str())
                            && filename.ends_with(".zip")
                    );
                }
            }

            let overwrite_behavior = if let Some(existing) = existing.get(&idx) {
                // Resolve old path, check if it exists. If no, errors
                let existing_path = std::path::Path::new(&existing);
                if !tokio::fs::try_exists(existing_path).await? {
                    return Err(anyhow::anyhow!(
                        "Existing content ({}, source {}) refers to nonexisting path: {}",
                        id,
                        idx + 1,
                        existing,
                    ));
                }

                // TODO: warns about as-is mode + relative
                let existing_full_path = if existing_path.is_absolute() {
                    existing_path.to_path_buf()
                } else {
                    session.get_pixiv_base_dir()?.join(existing_path)
                };

                match self.on_existing {
                    OnExistingBehavior::Skip => unreachable!(),
                    OnExistingBehavior::Verify => DownloadOverwriteBehavior::Compare {
                        old: existing_full_path,
                    },
                    OnExistingBehavior::Overwrite => DownloadOverwriteBehavior::Overwrite {
                        old: Some(existing_full_path),
                    },
                }
            } else {
                DownloadOverwriteBehavior::Free
            };

            if !self.dry_run {
                match self
                    .download_file(session, url, filename, overwrite_behavior)
                    .await?
                {
                    DownloadResult::Unchanged { size } => {
                        tracing::info!(
                            "Source {}/{}: Unchanged ({} bytes), refresh",
                            idx + 1,
                            tot_len,
                            size
                        );
                        let Some(old) = existing.get(&idx) else {
                            unreachable!();
                        };
                        assert!(
                            db.update_image_path_refresh(&old).await?,
                            "Fail to refresh, possible db race"
                        );
                    }
                    DownloadResult::Written {
                        written_path,
                        final_path,
                        old,
                        ..
                    } => {
                        tracing::info!(
                            "Source {}/{}: Saved to {}",
                            idx + 1,
                            tot_len,
                            final_path.display()
                        );
                        if let Some(existing) = existing.get(&idx) {
                            match old {
                                crate::util::DownloadOldResult::Stale => {} // Does nothing
                                crate::util::DownloadOldResult::Overwritten => {
                                    // Delete old
                                    assert!(
                                        db.update_image_path_move(&existing, None).await?,
                                        "Fail to update path for overwritten, possible db race"
                                    );
                                }
                                crate::util::DownloadOldResult::Moved(new) => {
                                    let new = new.to_str().ok_or_else(|| {
                                        anyhow::anyhow!("Failed to convert path to UTF-8")
                                    })?;
                                    assert!(
                                        db.update_image_path_move(&existing, Some(new)).await?,
                                        "Fail to update path for moved, possible db race"
                                    );
                                }
                            }
                        }

                        let written_path = written_path
                            .to_str()
                            .ok_or_else(|| anyhow::anyhow!("Failed to convert path to UTF-8"))?;

                        let (width, height) =
                            tokio::task::block_in_place(|| src.get_dimension(final_path))?;
                        db.insert_image(
                            id,
                            idx,
                            url,
                            written_path,
                            width,
                            height,
                            src.ugoira_frames(),
                        )
                        .await?;
                    }
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
        overwrite_behavior: DownloadOverwriteBehavior,
    ) -> anyhow::Result<DownloadResult> {
        crate::util::download_then_persist(
            PixivRequest(session),
            session.get_pixiv_base_dir()?,
            filename,
            self.database_path_format,
            url,
            overwrite_behavior,
            self.progress,
        )
        .await
    }
}
