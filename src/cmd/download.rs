use std::path::PathBuf;

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

#[derive(Args)]
pub struct Download {
    /// Illustration ID
    id: u64,

    /// Dry run, only fech and print the info
    #[arg(long)]
    dry_run: bool,

    /// Create base directory if not exist
    #[arg(short = 'p', long)]
    mkdir: bool,

    /// Base directory to save the illustration
    ///
    /// The illustrations will be saved as `<base_dir>/<illust_id>_p<page>.<ext>`
    #[arg(short, long, default_value = "images")]
    base_dir: String,

    /// Whether to skip canonicalizing the path written into database
    #[arg(long, value_enum, default_value_t = DatabasePathFormat::Absolute)]
    database_path_format: DatabasePathFormat,
}

impl Download {
    pub async fn run(self, session: &crate::config::Session) -> anyhow::Result<()> {
        if self.mkdir {
            std::fs::create_dir_all(&self.base_dir)?;
        }

        // FIXME: handle ugoira

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
                let mut tmp_file = NamedTempFile::with_prefix_in("pixivdwn_", &self.base_dir)?;
                let mut buffered_file = std::io::BufWriter::new(tmp_file.as_file_mut());
                crate::image::download(
                    session,
                    crate::image::DownloadSource::Pixiv,
                    url,
                    &mut buffered_file,
                )
                .await?;
                drop(buffered_file);
                let mut final_path = PathBuf::from(&self.base_dir);
                final_path.push(filename);

                let written_path = match self.database_path_format {
                    DatabasePathFormat::Inline => PathBuf::from(filename),
                    DatabasePathFormat::AsIs => final_path.clone(),
                    DatabasePathFormat::Absolute => final_path.canonicalize()?,
                };

                tmp_file.persist(&final_path)?;
                tracing::info!("Saved to {:?}", final_path);
                let written_path = written_path.to_str().ok_or_else(|| {
                    anyhow::anyhow!("Failed to convert path")
                })?;
                // FIXME: is this reliable for ugoira?
                crate::db::update_image(self.id, idx, url, written_path, page.width, page.height).await?;
            }
        }

        Ok(())
    }
}
