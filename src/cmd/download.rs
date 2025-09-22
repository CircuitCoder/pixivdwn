use clap::Args;
use tempfile::NamedTempFile;

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
}

impl Download {
    pub async fn run(self, session: &crate::config::Session) -> anyhow::Result<()> {
        if self.mkdir {
            std::fs::create_dir_all(&self.base_dir)?;
        }

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
            assert!(filename.starts_with(format!("{}_p{}.", self.id, idx).as_str()));

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
                let mut final_path = std::path::PathBuf::from(&self.base_dir);
                final_path.push(filename);
                tmp_file.persist(&final_path)?;
                tracing::info!("Saved to {:?}", final_path);
            }
        }

        Ok(())
    }
}
