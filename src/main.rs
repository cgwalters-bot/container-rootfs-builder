#![forbid(unsafe_code)]
//! Plan and build source-backed Fedora container root filesystems.

mod catalog;
mod cli;
mod kiwi;
mod provenance;
mod pungi;
mod recipe;
mod rootfs;
mod source;

use anyhow::{Context, Result, bail};
use clap::Parser;

use crate::cli::{Cli, Command, SourceCommand};

fn run(cli: Cli) -> Result<()> {
    match cli.command {
        Some(Command::Source(SourceCommand::Inspect(args))) => {
            println!(
                "{}",
                serde_json::to_string_pretty(&source::inspect(&args.from, &args.source_dirs)?)?
            );
            return Ok(());
        }
        Some(Command::BuildRootfs(args)) => return rootfs::build(args),
        None => {}
    }
    let from = cli
        .from
        .as_deref()
        .context("--from is required unless using `source inspect`")?;
    let target = cli
        .target_rootfs
        .as_deref()
        .context("target rootfs is required unless using `source inspect`")?;
    let packages = cli
        .recipe
        .as_deref()
        .map(recipe::read)
        .transpose()?
        .unwrap_or_default();
    let plan = catalog::plan(from, target, packages)?;
    if !cli.plan {
        bail!(
            "all recognized image references are plan-only; pass --plan to print the source mapping without building a rootfs"
        )
    }
    println!("{}", serde_json::to_string_pretty(&plan)?);
    Ok(())
}

fn main() {
    if let Err(error) = run(Cli::parse()) {
        eprintln!("container-rootfs-builder: {error:#}");
        std::process::exit(1)
    }
}
