//! Command-line argument definitions.

use std::path::PathBuf;

use clap::Parser;

/// Top-level command-line arguments.
#[derive(Parser, Debug)]
#[command(about = "Plan source-backed Fedora container rootfs work", version)]
pub(crate) struct Cli {
    #[command(subcommand)]
    pub(crate) command: Option<Command>,
    #[arg(long)]
    pub(crate) from: Option<String>,
    #[arg(long)]
    pub(crate) recipe: Option<PathBuf>,
    #[arg(long)]
    pub(crate) plan: bool,
    pub(crate) target_rootfs: Option<PathBuf>,
}

/// Top-level subcommands.
#[derive(clap::Subcommand, Debug)]
pub(crate) enum Command {
    #[command(subcommand)]
    Source(SourceCommand),
    /// Build one of the explicitly supported Fedora root filesystems.
    BuildRootfs(BuildRootfsArgs),
}

/// Commands that inspect source descriptions.
#[derive(clap::Subcommand, Debug)]
pub(crate) enum SourceCommand {
    Inspect(InspectArgs),
}

/// Arguments for source inspection.
#[derive(clap::Args, Debug)]
pub(crate) struct InspectArgs {
    #[arg(long)]
    pub(crate) from: String,
    #[arg(long = "source-dir", required = true)]
    pub(crate) source_dirs: Vec<PathBuf>,
}

/// Arguments for native rootfs construction.
#[derive(clap::Args, Debug)]
pub(crate) struct BuildRootfsArgs {
    /// Exact supported Fedora OCI reference.
    #[arg(long)]
    pub(crate) from: String,
    /// A new, nonexistent directory to receive the root filesystem.
    #[arg(long)]
    pub(crate) target: PathBuf,
    /// Mounted Fedora repository image root used by rpm-ostree for DNF configuration.
    #[arg(long)]
    pub(crate) source_root: PathBuf,
    /// Explicitly allow the selected backend to mutate the repository source root.
    #[arg(long)]
    pub(crate) source_root_rw: bool,
    /// Permit an explicitly requested cross-release build (advanced use).
    #[arg(long)]
    pub(crate) allow_cross_release: bool,
}
