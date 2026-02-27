use std::{
    fs::File,
    io::{BufRead, Read},
    path::{Path, PathBuf},
    str::FromStr,
};

use clap::Args;
use sha2::{Digest, Sha256};
use sqlx::{Column, Row, TypeInfo, sqlite::SqliteRow};

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

pub enum DownloadOverwriteBehavior {
    Compare {
        /// The old file to compare against
        old: PathBuf,
    },
    Overwrite {
        /// Optionally, the old file to overwrite
        old: Option<PathBuf>,
    },
    Free, // No file present
}

pub enum DownloadOldResult {
    Stale,          // Old file unchanged, or nonexistent
    Overwritten,    // Old file is overwritten
    Moved(PathBuf), // Old file is moved away
}

pub enum DownloadResult {
    Written {
        // FIXME: move written_path and fmt into caller to handle
        written_path: PathBuf,
        final_path: PathBuf,
        size: usize,

        old: DownloadOldResult,
    },

    Unchanged {
        size: usize,
    },
}

pub async fn download_then_persist<R: RequestArgumenter>(
    req_arg: R,
    base_dir: &Path,
    filename: &str,
    fmt: DatabasePathFormat,
    url: &str,
    overwrite_behavior: DownloadOverwriteBehavior,
    show_progress: bool,
) -> anyhow::Result<DownloadResult> {
    let (tmp_file, size, digest) =
        crate::data::file::download_to_tmp(req_arg, base_dir, url, show_progress).await?;

    let mut final_path = base_dir.canonicalize()?;
    final_path.push(filename);

    // Compare against old if requested
    let old = match overwrite_behavior {
        DownloadOverwriteBehavior::Compare { old } => {
            if !old.is_absolute() {
                return Err(anyhow::anyhow!(
                    "Old file path must be absolute for comparison"
                ));
            }

            // Compute hash for the old file
            let old_digest = tokio::task::block_in_place(|| -> anyhow::Result<[u8; 32]> {
                let mut old_file = File::open(&old)?;
                let mut old_digest = Sha256::new();
                let mut old_reader = std::io::BufReader::new(&mut old_file);
                std::io::copy(&mut old_reader, &mut old_digest)?;
                Ok(old_digest.finalize().into())
            })?;

            if old_digest == digest {
                // No change, skip writing
                return Ok(DownloadResult::Unchanged { size });
            }

            // Now decide if old needs to move
            let old_need_moving = final_path == old.canonicalize()?;
            if old_need_moving {
                // Changed, rename old file, append hash as suffix
                let mut old_moved = old.clone();
                let old_ext = old.extension();
                old_moved.set_extension(hex::encode(old_digest));
                if let Some(ext) = old_ext {
                    old_moved.add_extension(ext);
                }

                // Almost certainly that they are on the same filesystem
                tracing::info!("Moving: {} -> {}", old.display(), old_moved.display());
                tokio::fs::rename(&old, &old_moved).await?;
                DownloadOldResult::Moved(old_moved)
            } else {
                DownloadOldResult::Stale
            }
        }
        DownloadOverwriteBehavior::Overwrite { old: None } => {
            match tokio::fs::remove_file(&final_path).await {
                Ok(_) => {
                    tracing::info!("Removed: {}", final_path.display());
                    DownloadOldResult::Overwritten
                }
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                    // No existing file, just write
                    DownloadOldResult::Stale
                }
                Err(e) => {
                    return Err(e.into());
                }
            }
        }
        DownloadOverwriteBehavior::Overwrite { old: Some(old) } => {
            if !old.is_absolute() {
                return Err(anyhow::anyhow!(
                    "Old file path must be absolute for overwriting"
                ));
            }

            if final_path == old.canonicalize()? {
                tokio::fs::remove_file(&final_path).await?;
                tracing::info!("Removed: {}", final_path.display());
                DownloadOldResult::Overwritten
            } else {
                DownloadOldResult::Stale
            }
        }
        DownloadOverwriteBehavior::Free => DownloadOldResult::Stale,
    };

    if tokio::fs::try_exists(&final_path).await? {
        return Err(anyhow::anyhow!(
            "Final path already exists and not moved / overwritten: {}",
            final_path.display()
        ));
    }

    tmp_file.persist(&final_path)?;
    tracing::debug!("Saved to {}", final_path.display());

    let written_path = match fmt {
        DatabasePathFormat::Inline => PathBuf::from(filename),
        DatabasePathFormat::AsIs => final_path.clone(),
        DatabasePathFormat::Absolute => final_path.canonicalize()?,
    };

    Ok(DownloadResult::Written {
        written_path,
        final_path,
        size,

        old,
    })
}

#[derive(clap::ValueEnum, Clone, Copy, PartialEq, Eq)]
pub enum TerminationCondition {
    /// Terminate when an already existing illustration is encountered
    OnHit,

    /// Terminate until no more illustrations are available
    UntilEnd,
}

pub fn get_image_dim(
    mut file: impl std::io::Read,
    path: impl AsRef<Path>,
    mime_type: Option<&str>,
) -> anyhow::Result<(u32, u32)> {
    let image_fmt = if let Some(mime_type) = mime_type {
        image::ImageFormat::from_mime_type(mime_type)
            .ok_or_else(|| anyhow::anyhow!("Unknown mime type: {}", mime_type))?
    } else {
        image::ImageFormat::from_path(path.as_ref())?
    };

    let mut file_content = Vec::new();
    file.read_to_end(&mut file_content)?;
    let file_content = std::io::Cursor::new(file_content);

    let image = image::ImageReader::with_format(file_content, image_fmt);
    let (width, height) = image.into_dimensions()?;
    Ok((width, height))
}

#[derive(Args)]
#[group(multiple = false, required = true)]
pub struct DownloadIdSrc<U: FromStr>
where
    <U as FromStr>::Err: Into<Box<dyn std::error::Error + std::marker::Send + Sync + 'static>>,
    U: Send + Sync + Clone + 'static,
{
    /// IDs in argument
    pub id: Option<Vec<U>>,

    /// Reading illustration IDs from a file (`-` for STDIN)
    #[arg(short, long)]
    pub list: Option<String>,
}

impl<U: FromStr> DownloadIdSrc<U>
where
    <U as FromStr>::Err: Into<Box<dyn std::error::Error + std::marker::Send + Sync + 'static>>,
    U: Send + Sync + Clone + 'static,
{
    pub fn read(&self) -> anyhow::Result<Box<dyn Iterator<Item = anyhow::Result<U>>>> {
        if let Some(ref ids) = self.id {
            let ids = ids.clone();
            Ok(Box::new(ids.into_iter().map(Ok)))
        } else if let Some(ref file) = self.list {
            read_spec::<U>(file).map(|it| Box::new(it) as _)
        } else {
            panic!("Trying to iterate DownloadIdSrc without any source");
        }
    }
}

fn read_spec<T: FromStr>(
    src: &str,
) -> anyhow::Result<impl Iterator<Item = anyhow::Result<T>> + 'static> {
    let reader: Box<dyn Read> = if src == "-" {
        Box::new(std::io::stdin())
    } else {
        Box::new(std::fs::File::open(src)?)
    };
    let buf_reader = std::io::BufReader::new(reader);
    Ok(buf_reader.lines().map(|line| {
        let line = line?;
        line.parse::<T>()
            .map_err(|_| anyhow::anyhow!("Failed to parse line: {}", line))
    }))
}

pub fn db_row_to_json(
    row: SqliteRow,
) -> anyhow::Result<serde_json::Map<String, serde_json::Value>> {
    let mut map = serde_json::Map::new();
    for col in row.columns() {
        let name = col.name();
        let ordinal = col.ordinal();
        let ty = col.type_info();
        let val: serde_json::Value = match ty.name() {
            "NULL" => serde_json::Value::Null,
            "INTEGER" => row.get::<i64, _>(ordinal).into(),
            "REAL" => row.get::<f64, _>(ordinal).into(),
            "TEXT" => row.get::<String, _>(ordinal).into(),
            "BOOLEAN" => row.get::<bool, _>(ordinal).into(),
            _ => return Err(anyhow::anyhow!("Unsupported column type: {}", ty.name())),
        };
        map.insert(name.to_string(), val);
    }
    Ok(map)
}
