use std::{
    io::{BufRead, Read},
    path::{Path, PathBuf},
    str::FromStr,
};

use clap::Args;
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

pub struct DownloadResult {
    pub written_path: PathBuf,
    pub final_path: PathBuf,
    pub size: u64,
}

pub async fn download_then_persist<R: RequestArgumenter>(
    req_arg: R,
    base_dir: &Path,
    filename: &str,
    fmt: DatabasePathFormat,
    url: &str,
    show_progress: bool,
) -> anyhow::Result<DownloadResult> {
    let (tmp_file, size) =
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
        size,
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
