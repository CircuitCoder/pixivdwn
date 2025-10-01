use std::path::PathBuf;

use crate::data::RequestArgumenter;

#[derive(clap::ValueEnum, Clone, Copy)]
pub enum DatabasePathFormat {
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

pub struct DownloadResult {
    pub written_path: PathBuf,
    pub final_path: PathBuf,
}

pub async fn download_then_persist<R: RequestArgumenter>(
    req_arg: R,
    base_dir: &str,
    filename: &str,
    fmt: DatabasePathFormat,
    url: &str,
    show_progress: bool,
) -> anyhow::Result<DownloadResult> {
    let tmp_file =
        crate::data::file::download_to_tmp(req_arg, base_dir, url, show_progress).await?;

    let mut final_path = PathBuf::from(base_dir);
    final_path.push(filename);
    tmp_file.persist(&final_path)?;
    tracing::info!("Saved to {}", final_path.display());

    let written_path = match fmt {
        DatabasePathFormat::Inline => PathBuf::from(filename),
        DatabasePathFormat::AsIs => final_path.clone(),
        DatabasePathFormat::Absolute => final_path.canonicalize()?,
    };

    Ok(DownloadResult {
        written_path,
        final_path,
    })
}
