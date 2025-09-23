use std::{io::Read, path::PathBuf};

use clap::Args;
use tempfile::NamedTempFile;

#[derive(clap::ValueEnum, Clone, Copy)]
enum DatabasePathFormat {
    /// Only store the path relative to the base.
    ///
    /// This make it easier to move the base directory with images inside them.
    Inline,

    /// Store the path as-is after concating with the base directory,
    ///
    /// Not recommended, but may be useful in some cases.
    AsIs,

    /// Store the absolute path to the image.
    ///
    /// Useful if the base directory is often changed, but the image themselves are not moved.
    Absolute,
}

#[derive(clap::ValueEnum, Clone, Copy)]
enum DownloadType {
    /// Force download as ugoira
    Ugoira,

    /// Force download as image (or manga)
    Image,
}

#[derive(Args)]
pub struct Download {
    /// Illustration ID
    id: u64,

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

    /// Whether to skip canonicalizing the path written into database
    #[arg(long, value_enum, default_value_t = DatabasePathFormat::Absolute)]
    database_path_format: DatabasePathFormat,

    /// Force download type.
    #[arg(short = 't', long)]
    download_type: Option<DownloadType>,

    /// Show progress bar. The download speed is based on the *UNZIPPED* stream, so don't be surprised if it exceeds your bandwidth.
    #[arg(short, long)]
    progress: bool,
}

impl Download {
    pub async fn run(self, session: &crate::config::Session) -> anyhow::Result<()> {
        if self.mkdir {
            std::fs::create_dir_all(&self.base_dir)?;
        }

        let illust_type = crate::db::get_illust_type(self.id).await?.ok_or_else(|| {
            anyhow::anyhow!(
                "{} not found in DB. Please run `pixivdwn illust {}` first.",
                self.id,
                self.id
            )
        })?;
        let induced_download_type = match illust_type {
            crate::data::IllustType::Ugoira => DownloadType::Ugoira,
            _ => DownloadType::Image,
        };
        let download_type = self.download_type.unwrap_or(induced_download_type);

        match download_type {
            DownloadType::Image => {
                let pages = crate::data::get_illust_pages(session, self.id).await?;
                let tot_pages = pages.len();
                tracing::info!("Downloading {} pages...", tot_pages);
                for (idx, page) in pages.iter().enumerate() {
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
                        filename.starts_with(format!("{}_p{}.", self.id, idx).as_str())
                            || filename.starts_with(format!("{}_ugoira{}.", self.id, idx).as_str())
                    );

                    if !self.dry_run {
                        let (written_path, _) = self.download_file(session, url, filename).await?;
                        let written_path = written_path
                            .to_str()
                            .ok_or_else(|| anyhow::anyhow!("Failed to convert path"))?;
                        crate::db::update_image(
                            self.id,
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
                let meta = crate::data::get_illust_ugoira_meta(session, self.id).await?;
                tracing::info!("Downloading ugoira...");
                let url = &meta.original_src;
                let filename = url.split('/').last().unwrap();
                tracing::info!("Ugoira pack {} from {}", filename, url);
                assert!(
                    filename.starts_with(format!("{}_ugoira", self.id).as_str())
                        && filename.ends_with(".zip")
                );

                if !self.dry_run {
                    let (written_path, final_path) =
                        self.download_file(session, url, filename).await?;
                    let mut archive = zip::ZipArchive::new(std::fs::File::open(&final_path)?)?;
                    let mut file = archive.by_name(&meta.frames[0].file)?;
                    let mut file_content = Vec::new();
                    file.read_to_end(&mut file_content)?;
                    let file_content = std::io::Cursor::new(file_content);

                    let image_fmt = image::ImageFormat::from_mime_type(&meta.mime_type)
                        .ok_or_else(|| anyhow::anyhow!("Unknown mime type: {}", meta.mime_type))?;
                    let image = image::ImageReader::with_format(file_content, image_fmt);
                    let (width, height) = image.into_dimensions()?;

                    let written_path = written_path
                        .to_str()
                        .ok_or_else(|| anyhow::anyhow!("Failed to convert path"))?;
                    crate::db::update_image(
                        self.id,
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
    ) -> anyhow::Result<(PathBuf, PathBuf)> {
        let mut tmp_file = NamedTempFile::with_prefix_in("pixivdwn_", &self.base_dir)?;
        let mut buffered_file = std::io::BufWriter::new(tmp_file.as_file_mut());
        crate::image::download(
            session,
            crate::image::DownloadSource::Pixiv,
            url,
            &mut buffered_file,
            self.progress,
        )
        .await?;
        drop(buffered_file);
        let mut final_path = PathBuf::from(&self.base_dir);
        final_path.push(filename);

        tmp_file.persist(&final_path)?;
        tracing::info!("Saved to {:?}", final_path);

        let written_path = match self.database_path_format {
            DatabasePathFormat::Inline => PathBuf::from(filename),
            DatabasePathFormat::AsIs => final_path.clone(),
            DatabasePathFormat::Absolute => final_path.canonicalize()?,
        };

        Ok((written_path, final_path))
    }
}
