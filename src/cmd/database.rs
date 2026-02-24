use std::path::{Path, PathBuf};

use clap::{Args, Subcommand};

use crate::{cmd::fanbox, config::Session, util::DatabasePathFormat};

#[derive(Args)]
pub struct Database {
    #[command(subcommand)]
    cmd: DatabaseCmd,
}

#[derive(Args)]
pub struct FileArgs {
    #[command(subcommand)]
    cmd: FileCmd,
}

impl FileArgs {
    pub async fn run(&self, session: &Session, db: &crate::db::Database) -> anyhow::Result<()> {
        match self.cmd {
            FileCmd::Fsck(ref args) => args.run(session, db).await?,
            FileCmd::Canonicalize(ref args) => args.run(session, db).await?,
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
    Fsck(FileFsckArgs),

    /// Canonicalize downloaded paths
    Canonicalize(FileCanonicalizeArgs),
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

    /// Besides skipping moving file, also don't check if file is already at destination
    #[arg(long, requires = "skip_file")]
    skip_file_without_existence_check: bool,

    /// Perform a dry run
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
    pub async fn run(&self, session: &Session, db: &crate::db::Database) -> anyhow::Result<()> {
        let mut failed = 0usize;
        if !self.skip_pixiv {
            let entries = db.query_image_paths().await?;
            for ent in entries {
                if let Some(p) = ent.path
                    && !Self::check(&p, session.get_pixiv_base_dir()).await?
                {
                    failed += 1;
                    tracing::error!("Missing pixiv image {} ({}_p{})", p, ent.id.0, ent.id.1);
                }
            }
        }

        if !self.skip_fanbox_images {
            let entries = db.query_fanbox_image_paths().await?;
            for ent in entries {
                if let Some(p) = ent.path
                    && !Self::check(&p, session.get_fanbox_base_dir()).await?
                {
                    failed += 1;
                    tracing::error!(
                        "Missing fanbox image {} ({}_{}_{})",
                        p,
                        ent.id.1,
                        ent.id.2,
                        ent.id.0
                    );
                }
            }
        }

        if !self.skip_fanbox_files {
            let entries = db.query_fanbox_file_paths().await?;
            for ent in entries {
                if let Some(p) = ent.path
                    && !Self::check(&p, session.get_fanbox_base_dir()).await?
                {
                    failed += 1;
                    tracing::error!(
                        "Missing fanbox file {} ({}_{}_{})",
                        p,
                        ent.id.1,
                        ent.id.2,
                        ent.id.0
                    );
                }
            }
        }

        if failed > 0 {
            Err(anyhow::anyhow!("{} files missing", failed))
        } else {
            Ok(())
        }
    }

    async fn check(path: &str, base_dir: anyhow::Result<&PathBuf>) -> anyhow::Result<bool> {
        // Path may be absolute or relative
        let full_path = if std::path::Path::new(path).is_absolute() {
            std::path::PathBuf::from(path)
        } else if let Ok(base_dir) = base_dir {
            let mut p = base_dir.clone();
            p.push(path);
            p
        } else {
            return Err(anyhow::anyhow!(
                "Relative path {} requires specified base dir: {}",
                path,
                base_dir.unwrap_err()
            ));
        };
        tracing::debug!("Checking path {}", full_path.display());

        Ok(full_path.try_exists()?)
    }
}

impl FileCanonicalizeArgs {
    pub async fn run(&self, session: &Session, db: &crate::db::Database) -> anyhow::Result<()> {
        if !self.skip_pixiv {
            let entries = db.query_image_paths().await?;
            let base_dir = session.get_pixiv_base_dir()?;
            let base_dir_old = self.base_dir_old.as_ref().unwrap_or(base_dir);
            for ent in entries {
                if let Some(cur) = ent.path {
                    // Use original filename for images
                    // This also handles modified filenames (e.g. hash suffixes for older versions)
                    let filename = cur.split('/').last().unwrap();
                    let written_path = self.adjust(&cur, base_dir_old, &filename, base_dir).await?;
                    let new_path_str = &written_path
                        .to_str()
                        .ok_or_else(|| anyhow::anyhow!("Failed to convert path"))?;
                    if !self.skip_db && !self.dry_run {
                        db.update_image_path_move(
                            &cur,
                            Some(*new_path_str),
                        )
                        .await?;
                    }
                }
            }
        }

        if !self.skip_fanbox_images {
            let base_dir = session.get_fanbox_base_dir()?;
            let base_dir_old = self.fanbox_base_dir_old.as_ref().unwrap_or(base_dir);
            let entries = db.query_fanbox_image_paths().await?;
            for ent in entries {
                if let Some(cur) = ent.path {
                    let filename = fanbox::get_download_spec(
                        db,
                        fanbox::FanboxAttachmentType::Image,
                        &ent.id.0,
                    )
                    .await?
                    .1;
                    let written_path = self.adjust(&cur, base_dir_old, &filename, base_dir).await?;
                    if !self.skip_db && !self.dry_run {
                        db.update_fanbox_image_path(
                            &ent.id.0,
                            &written_path
                                .to_str()
                                .ok_or_else(|| anyhow::anyhow!("Failed to convert path"))?,
                        )
                        .await?;
                    }
                }
            }
        }

        if !self.skip_fanbox_files {
            let base_dir = session.get_fanbox_base_dir()?;
            let base_dir_old = self.fanbox_base_dir_old.as_ref().unwrap_or(base_dir);
            let entries = db.query_fanbox_file_paths().await?;
            for ent in entries {
                if let Some(cur) = ent.path {
                    let filename = fanbox::get_download_spec(
                        db,
                        fanbox::FanboxAttachmentType::File,
                        &ent.id.0,
                    )
                    .await?
                    .1;
                    let written_path = self.adjust(&cur, base_dir_old, &filename, base_dir).await?;
                    if !self.skip_db && !self.dry_run {
                        db.update_fanbox_file_path(
                            &ent.id.0,
                            &written_path
                                .to_str()
                                .ok_or_else(|| anyhow::anyhow!("Failed to convert path"))?,
                        )
                        .await?;
                    }
                }
            }
        }

        Ok(())
    }

    async fn adjust(
        &self,
        cur: &str,
        base_dir_old: &PathBuf,
        filename: &str,
        base_dir: &PathBuf,
    ) -> anyhow::Result<PathBuf> {
        let mut target_path = base_dir.clone();
        target_path.push(filename);
        // We use absolute here because the target file does not exist yet
        let target_path_full = std::path::absolute(target_path.as_path())?;

        let cur_path = std::path::Path::new(cur);
        let cur_full_path = if cur_path.is_absolute() {
            std::path::PathBuf::from(cur)
        } else {
            let mut p = base_dir_old.clone();
            p.push(cur);
            p
        }
        .canonicalize();
        let cur_resolved_path = cur_full_path
            .as_ref()
            .map(PathBuf::as_path)
            .unwrap_or(cur_path);

        if cur_resolved_path != target_path {
            // Check file existence requirement
            let target_exists = target_path.exists();
            tracing::info!(
                "{} -> {}",
                cur_resolved_path.display(),
                target_path.display()
            );
            if !self.skip_file && !cur_full_path.is_ok() {
                return Err(anyhow::anyhow!(
                    "{} -> {}: Source file does not exist",
                    cur_resolved_path.display(),
                    target_path.display()
                ));
            } else if !self.skip_file && target_exists && !self.overwrite {
                return Err(anyhow::anyhow!(
                    "{} -> {}: Target file already exists",
                    cur_resolved_path.display(),
                    target_path.display()
                ));
            } else if self.skip_file && !self.skip_file_without_existence_check && !target_exists {
                return Err(anyhow::anyhow!(
                    "{} -> {}: Target file does not exist in skip file mode",
                    cur_resolved_path.display(),
                    target_path.display()
                ));
            }

            if !self.dry_run {
                if !self.skip_file {
                    if target_exists {
                        tracing::warn!("Overwriting existing file {}", target_path.display());
                    }
                    Self::mv(&cur_full_path.unwrap(), &target_path).await?;
                } else if let Ok(cur_full_path) = cur_full_path
                    && cur_full_path.exists()
                {
                    tracing::warn!(
                        "Source exists in skip file mode: {}",
                        cur_full_path.display()
                    );
                }
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
    pub async fn run(self, session: &Session, dburl: &str) -> anyhow::Result<()> {
        match self.cmd {
            DatabaseCmd::Setup => self.setup(dburl).await,
            DatabaseCmd::File(file) => {
                let db = crate::db::Database::load(dburl).await?;
                file.run(session, &db).await
            }
        }
    }

    pub async fn setup(self, dburl: &str) -> anyhow::Result<()> {
        crate::db::Database::setup(dburl).await?;
        Ok(())
    }
}
