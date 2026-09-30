//! Native root filesystem validation and backend execution.

use std::{
    path::{Path, PathBuf},
    process::{Command, Stdio},
};

use anyhow::{Context, Result, bail};

use crate::{
    catalog::{self, BuildEngine, CatalogSources},
    cli::BuildRootfsArgs,
};

const BOOTC_HELPER: &str = "/usr/local/bin/bootc-base-imagectl";

fn helper_path() -> PathBuf {
    std::env::var_os("CONTAINER_ROOTFS_BUILDER_HELPER")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(BOOTC_HELPER))
}

fn rpm_ostree_path() -> PathBuf {
    std::env::var_os("CONTAINER_ROOTFS_BUILDER_RPM_OSTREE")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("rpm-ostree"))
}

fn validate_new_target(target: &Path) -> Result<()> {
    if !target.is_absolute() {
        bail!(
            "build-rootfs target must be an absolute path: {}",
            target.display()
        );
    }
    match std::fs::symlink_metadata(target) {
        Ok(_) => bail!(
            "refusing to overwrite existing build-rootfs target: {}",
            target.display()
        ),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => {
            return Err(error).with_context(|| format!("checking target {}", target.display()));
        }
    }
    let parent = target
        .parent()
        .context("build-rootfs target has no parent")?;
    if !parent.is_dir() {
        bail!(
            "build-rootfs target parent is not a directory: {}",
            parent.display()
        );
    }
    Ok(())
}

fn parse_os_release(os_release: &str) -> Result<String> {
    let id = os_release.lines().find_map(|line| {
        line.strip_prefix("ID=")
            .map(|value| value.trim_matches('"').to_string())
    });
    if id.as_deref() != Some("fedora") {
        bail!(
            "source root /etc/os-release is not Fedora (expected ID=fedora); refusing to guess the repository release"
        );
    }
    let release = os_release.lines().find_map(|line| {
        line.strip_prefix("VERSION_ID=")
            .map(|value| value.trim_matches('"').to_string())
    });
    release.filter(|value| !value.is_empty()).context(
        "source root /etc/os-release has no VERSION_ID; refusing to guess the repository release",
    )
}

fn source_release(source_root: &Path) -> Result<String> {
    let os_release = std::fs::read_to_string(source_root.join("etc/os-release"))
        .with_context(|| format!("reading {}/etc/os-release", source_root.display()))?;
    parse_os_release(&os_release)
}

fn validate_source_target(source_root: &Path, target: &Path) -> Result<()> {
    if source_root == Path::new("/") {
        bail!("refusing to use / as the build-rootfs source root");
    }
    let source = std::fs::canonicalize(source_root)
        .with_context(|| format!("canonicalizing source root {}", source_root.display()))?;
    let parent = target
        .parent()
        .context("build-rootfs target has no parent")?;
    let target_parent = std::fs::canonicalize(parent)
        .with_context(|| format!("canonicalizing target parent {}", parent.display()))?;
    let canonical_target =
        target_parent.join(target.file_name().context("target has no filename")?);
    if canonical_target == source || canonical_target.starts_with(&source) {
        bail!(
            "refusing source/target overlap: target {} is inside source root {}",
            target.display(),
            source.display()
        );
    }
    Ok(())
}

pub(crate) fn build(args: BuildRootfsArgs) -> Result<()> {
    let entry = catalog::entry(&args.from)?;
    if entry.engine == BuildEngine::PlanOnly {
        bail!(
            "build-rootfs does not support {}; it remains plan-only",
            args.from
        );
    }
    let requested_release = entry.declared_release;
    validate_new_target(&args.target)?;
    if !args.source_root.is_absolute() || !args.source_root.is_dir() {
        bail!(
            "build-rootfs source root must be an existing absolute directory: {}",
            args.source_root.display()
        );
    }
    validate_source_target(&args.source_root, &args.target)?;
    let actual_release = source_release(&args.source_root)?;
    if actual_release != requested_release && !args.allow_cross_release {
        bail!(
            "source root is Fedora {actual_release}, but --from requests Fedora {requested_release}; use matching repository image (cross-release builds are advanced and must be explicit)"
        );
    }
    if !args.source_root_rw {
        bail!(
            "source root is read-only by default; pass --source-root-rw only for a disposable repository source root because the selected backend may mutate it"
        );
    }
    match entry.engine {
        BuildEngine::PlanOnly => unreachable!(),
        BuildEngine::BootcBaseImagectl { manifest } => {
            let helper = helper_path();
            let status = Command::new(&helper)
                .arg("build-rootfs")
                .arg(format!("--manifest={manifest}"))
                .arg(&args.source_root)
                .arg(&args.target)
                .status()
                .with_context(|| format!("running embedded bootc helper {}", helper.display()))?;
            if !status.success() {
                bail!("embedded bootc helper failed with {status}");
            }
        }
        BuildEngine::RpmOstreeRootfs => {
            let CatalogSources::One(source) = entry.sources else {
                unreachable!("rpm-ostree catalog entry must select one source")
            };
            let manifest = Path::new(source.checkout_root).join(source.entrypoint);
            let rpm_ostree = rpm_ostree_path();
            let print_status = Command::new(&rpm_ostree)
                .args(["compose", "tree", "--print-only"])
                .arg(format!("--source-root={}", args.source_root.display()))
                .arg(&manifest)
                .stdout(Stdio::null())
                .status()
                .with_context(|| {
                    format!(
                        "checking Atomic Desktop treefile and referenced files with {}",
                        rpm_ostree.display()
                    )
                })?;
            if !print_status.success() {
                bail!("rpm-ostree compose tree --print-only failed with {print_status}");
            }
            let status = Command::new(&rpm_ostree)
                .args(["compose", "rootfs"])
                .arg(format!("--source-root-rw={}", args.source_root.display()))
                .arg(&manifest)
                .arg(&args.target)
                .status()
                .with_context(|| {
                    format!(
                        "running experimental Atomic Desktop rootfs compose with {}",
                        rpm_ostree.display()
                    )
                })?;
            if !status.success() {
                bail!("rpm-ostree compose rootfs failed with {status}");
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use anyhow::Result;

    use super::parse_os_release;

    #[test]
    fn parses_os_release_without_io() -> Result<()> {
        assert_eq!(
            parse_os_release("NAME=Fedora\nID=fedora\nVERSION_ID=\"44\"\n")?,
            "44"
        );
        assert!(parse_os_release("ID=other\nVERSION_ID=44\n").is_err());
        assert!(parse_os_release("ID=fedora\n").is_err());
        Ok(())
    }
}
