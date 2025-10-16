use std::path::PathBuf;

use clap::{Args, Subcommand};

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
    Canonicalize {
        /// Skip updating db
        #[arg(long)]
        skip_db: bool,

        /// Skip moving file
        #[arg(long)]
        skip_file: bool,

        /// Equivlent to `--skip-db --skip-file`
        #[arg(long)]
        dry_run: bool,
    },

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
