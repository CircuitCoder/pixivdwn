use std::path::{Path, PathBuf};

use clap::{Args, Subcommand};

use crate::{cmd::fanbox, util::DatabasePathFormat};

#[derive(Args)]
pub struct Database {
    #[command(subcommand)]
    cmd: DatabaseCmd,
}

#[derive(Args)]
pub struct FileArgs {
    #[command(subcommand)]
    cmd: FileCmd,

    /// Base directory of save illustrations
    #[arg(long)]
    base_dir: Option<PathBuf>,

    /// Base directory of save fanbox files
    #[arg(long)]
    fanbox_base_dir: Option<PathBuf>
}

impl FileArgs {
    pub async fn run(&self) -> anyhow::Result<()> {
        match self.cmd {
            FileCmd::Fsck(ref args) => args.run(self).await?,
            FileCmd::Canonicalize(ref args) => args.run(self).await?,
            _ => unimplemented!(),
        }
        Ok(())
    }
}

#[derive(Subcommand)]
pub enum DatabaseCmd {
    /// Setup / migrate the database
    Setup,

    /// File management
    File(FileArgs),
}

#[derive(Subcommand)]
pub enum FileCmd {
    /// Check the existence of downloaded files
    Fsck(FileFsckArgs) ,

    /// Canonicalize downloaded paths
    Canonicalize(FileCanonicalizeArgs),

    /// Move download base. Done by directly moving the entire directory.
    /// This is more efficient than canonicalizing with a new base dir
    MvBase {
        /// Move pixiv base to
        #[arg(long)]
        to: Option<String>,

        /// Move fanbox base to
        #[arg(long)]
        fanbox_to: Option<String>,

        /// Skip updating db
        #[arg(long)]
        skip_db: bool,

        /// Skip moving file
        #[arg(long)]
        skip_file: bool,

        /// Equivlent to `--skip-db --skip-file`
        #[arg(long)]
        dry_run: bool,
    }
}

#[derive(Args)]
pub struct FileFsckArgs {
    /// Don't check pixiv images
    #[arg(long)]
    skip_pixiv: bool,

    /// Don't check fanbox images
    #[arg(long)]
    skip_fanbox_images: bool,

    /// Don't check fanbox files
    #[arg(long)]
    skip_fanbox_files: bool,
}

#[derive(Args)]
pub struct FileCanonicalizeArgs {
    /// Resulting path format
    #[arg(short, long, value_enum, default_value_t = DatabasePathFormat::Absolute)]
    format: DatabasePathFormat,

    /// Don't check pixiv images
    #[arg(long)]
    skip_pixiv: bool,

    /// Don't check fanbox images
    #[arg(long)]
    skip_fanbox_images: bool,

    /// Don't check fanbox files
    #[arg(long)]
    skip_fanbox_files: bool,

    /// Skip updating db
    #[arg(long)]
    skip_db: bool,

    /// Skip moving file
    #[arg(long)]
    skip_file: bool,

    /// Equivlent to `--skip-db --skip-file`
    #[arg(long)]
    dry_run: bool,

    /// Overwrite existing files
    #[arg(short = 'f', long)]
    overwrite: bool,

    /// Override old base directory
    #[arg(long)]
    base_dir_old: Option<PathBuf>,

    /// Override old base directory for fanbox files
    #[arg(long)]
    fanbox_base_dir_old: Option<PathBuf>,
}

impl FileFsckArgs {
    pub async fn run(&self, outer: &FileArgs) -> anyhow::Result<()> {
        let mut failed = 0usize;
        if !self.skip_pixiv {
            let entries = crate::db::query_image_paths().await?;
            for ent in entries {
                if let Some(p) = ent.path && !Self::check(&p, outer.base_dir.as_ref()).await? {
                    failed += 1;
                    tracing::error!("Missing pixiv image {} ({}_p{})", p, ent.id.0, ent.id.1);
                }
            }
        }

        if !self.skip_fanbox_images {
            let entries = crate::db::query_fanbox_image_paths().await?;
            for ent in entries {
                if let Some(p) = ent.path && !Self::check(&p, outer.fanbox_base_dir.as_ref()).await? {
                    failed += 1;
                    tracing::error!("Missing fanbox image {} ({}_{}_{})", p, ent.id.1, ent.id.2, ent.id.0);
                }
            }
        }

        if !self.skip_fanbox_files {
            let entries = crate::db::query_fanbox_file_paths().await?;
            for ent in entries {
                if let Some(p) = ent.path && !Self::check(&p, outer.fanbox_base_dir.as_ref()).await? {
                    failed += 1;
                    tracing::error!("Missing fanbox file {} ({}_{}_{})", p, ent.id.1, ent.id.2, ent.id.0);
                }
            }
        }

        if failed > 0 {
            Err(anyhow::anyhow!("{} files missing", failed))
        } else {
            Ok(())
        }
    }

    async fn check(path: &str, base_dir: Option<&PathBuf>) -> anyhow::Result<bool> {
        // Path may be absolute or relative
        let full_path = if std::path::Path::new(path).is_absolute() {
            std::path::PathBuf::from(path)
        } else if let Some(base_dir) = base_dir {
            let mut p = base_dir.clone();
            p.push(path);
            p
        } else {
            return Err(anyhow::anyhow!("Relative path {} requires specified base dir", path));
        };
        tracing::debug!("Checking path {}", full_path.display());

        Ok(full_path.try_exists()?)
    }
}

impl FileCanonicalizeArgs {
    pub async fn run(&self, outer: &FileArgs) -> anyhow::Result<()> {
        if !self.skip_pixiv {
            let entries = crate::db::query_image_paths().await?;
            let base_dir = outer.base_dir.as_ref().ok_or_else(|| anyhow::anyhow!("Pixiv base dir not specified"))?;
            let base_dir_old = self.base_dir_old.as_ref().unwrap_or(base_dir);
            for ent in entries {
                if let Some(cur) = ent.path {
                    // Use original filename for images
                    let filename = cur.split('/').last().unwrap();
                    let written_path = self.adjust(&cur, base_dir_old, &filename, base_dir).await?;
                    if !self.skip_db && !self.dry_run {
                        crate::db::update_image_path(ent.id, &written_path.to_str().ok_or_else(|| anyhow::anyhow!("Failed to convert path"))?).await?;
                    }
                }
            }
        }

        if !self.skip_fanbox_images {
            let base_dir = outer.fanbox_base_dir.as_ref().ok_or_else(|| anyhow::anyhow!("Fanbox base dir not specified"))?;
            let base_dir_old = self.fanbox_base_dir_old.as_ref().unwrap_or(base_dir);
            let entries = crate::db::query_fanbox_image_paths().await?;
            for ent in entries {
                if let Some(cur) = ent.path {
                    let filename = fanbox::get_download_spec(fanbox::FanboxAttachmentType::Image, &ent.id.0).await?.1;
                    let written_path = self.adjust(&cur, base_dir_old, &filename, base_dir).await?;
                    if !self.skip_db && !self.dry_run {
                        crate::db::update_fanbox_image_path(&ent.id.0, &written_path.to_str().ok_or_else(|| anyhow::anyhow!("Failed to convert path"))?).await?;
                    }
                }
            }
        }

        if !self.skip_fanbox_files {
            let base_dir = outer.fanbox_base_dir.as_ref().ok_or_else(|| anyhow::anyhow!("Fanbox base dir not specified"))?;
            let base_dir_old = self.fanbox_base_dir_old.as_ref().unwrap_or(base_dir);
            let entries = crate::db::query_fanbox_file_paths().await?;
            for ent in entries {
                if let Some(cur) = ent.path {
                    let filename = fanbox::get_download_spec(fanbox::FanboxAttachmentType::File, &ent.id.0).await?.1;
                    let written_path = self.adjust(&cur, base_dir_old, &filename, base_dir).await?;
                    if !self.skip_db && !self.dry_run {
                        crate::db::update_fanbox_file_path(&ent.id.0, &written_path.to_str().ok_or_else(|| anyhow::anyhow!("Failed to convert path"))?).await?;
                    }
                }
            }
        }

        Ok(())

    }

    async fn adjust(&self, cur: &str, base_dir_old: &PathBuf, filename: &str, base_dir: &PathBuf) -> anyhow::Result<PathBuf> {
        let mut target_path = base_dir.clone();
        target_path.push(filename);
        // We use absolute here because the target file does not exist yet
        let target_path_full = std::path::absolute(target_path.as_path())?;

        let cur_full_path = if std::path::Path::new(cur).is_absolute() {
            std::path::PathBuf::from(cur)
        } else {
            let mut p = base_dir_old.clone();
            p.push(cur);
            p
        }.canonicalize()?;

        if cur_full_path != target_path {
            tracing::info!("{} -> {}", cur_full_path.display(), target_path.display());
            if !self.dry_run && !self.skip_file {
                if target_path.exists() {
                    if !self.overwrite {
                        return Err(anyhow::anyhow!("Target path {} already exists", target_path.display()));
                    }
                    tracing::warn!("Overwriting existing file {}", target_path.display());
                }
                Self::mv(&cur_full_path, &target_path).await?;
            }
        }

        let written_path = match self.format {
            DatabasePathFormat::Inline => PathBuf::from(filename),
            DatabasePathFormat::AsIs => target_path,
            DatabasePathFormat::Absolute => target_path_full,
        };

        Ok(written_path)
    }

    async fn mv(from: impl AsRef<Path>, to: impl AsRef<Path>) -> anyhow::Result<()> {
        // First, try normal rename
        // FIXME: preserve mtime
        let result = tokio::fs::rename(from.as_ref(), to.as_ref()).await;
        if result.is_ok() {
            return Ok(());
        }

        // Check if it's cross-device error
        let e = result.unwrap_err();
        if e.kind() != std::io::ErrorKind::CrossesDevices {
            return Err(e.into());
        }

        // Do copy + remove
        tokio::fs::copy(from.as_ref(), to.as_ref()).await?;
        tokio::fs::remove_file(from.as_ref()).await?;
        Ok(())
    }
}

impl Database {
    pub async fn run(self) -> anyhow::Result<()> {
        match self.cmd {
            DatabaseCmd::Setup => self.setup().await,
            DatabaseCmd::File(file) => file.run().await,
        }
    }

    pub async fn setup(self) -> anyhow::Result<()> {
        crate::db::setup_db().await?;
        Ok(())
    }
}
