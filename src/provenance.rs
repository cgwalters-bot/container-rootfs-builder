//! Capability-scoped verification of recorded source manifests.

use std::{
    collections::{BTreeMap, HashSet},
    io::Read,
    path::{Component, Path, PathBuf},
};

use anyhow::{Context, Result, bail};
use cap_std::fs::{Dir, OpenOptions, OpenOptionsExt};
use serde::Deserialize;
use sha2::{Digest, Sha256};

use crate::catalog::SourceDefinition;

const PROVENANCE: &str = ".source-provenance.json";

#[derive(Debug, Deserialize)]
pub(crate) struct Provenance {
    source_url: String,
    expected_commit: String,
    pub(crate) source_commit: String,
    files: BTreeMap<String, String>,
}

pub(crate) fn safe_relative(path: &str) -> Result<&Path> {
    let relative = Path::new(path);
    if relative.is_absolute()
        || relative.components().any(|component| {
            matches!(
                component,
                Component::ParentDir | Component::RootDir | Component::Prefix(_)
            )
        })
    {
        bail!("unsafe source path {path:?}")
    }
    Ok(relative)
}

fn digest(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn lexical_target(base: &Path, target: &Path) -> Result<PathBuf> {
    if target.is_absolute() {
        bail!("source symlink target is absolute: {}", target.display());
    }
    let mut result = base.to_path_buf();
    for component in target.components() {
        match component {
            Component::CurDir => {}
            Component::Normal(value) => result.push(value),
            Component::ParentDir => {
                if !result.pop() {
                    bail!("source symlink target escapes source root");
                }
            }
            Component::RootDir | Component::Prefix(_) => unreachable!(),
        }
    }
    Ok(result)
}

fn resolve_directory_path(root: &Dir, relative: &Path) -> Result<PathBuf> {
    let mut current = PathBuf::new();
    let mut seen = HashSet::new();
    for component in relative.components() {
        let name = match component {
            Component::Normal(name) => name,
            _ => unreachable!(),
        };
        let candidate = current.join(name);
        let metadata = root
            .symlink_metadata(&candidate)
            .with_context(|| format!("reading source directory {}", candidate.display()))?;
        if metadata.file_type().is_symlink() {
            if !seen.insert(candidate.clone()) {
                bail!("source symlink loop at {}", candidate.display());
            }
            let parent = candidate.parent().unwrap_or_else(|| Path::new(""));
            let parent_dir = if parent.as_os_str().is_empty() {
                root.try_clone()?
            } else {
                root.open_dir(parent)?
            };
            let target = parent_dir.read_link(Path::new(candidate.file_name().unwrap()))?;
            current = lexical_target(parent, &target)?;
        } else {
            current = candidate;
        }
    }
    Ok(current)
}

fn resolve_source_path(root: &Dir, relative: &Path) -> Result<(PathBuf, bool)> {
    let parent = resolve_directory_path(root, relative.parent().unwrap_or_else(|| Path::new("")))?;
    let current = parent.join(
        relative
            .file_name()
            .context("source path has no filename")?,
    );
    let metadata = root
        .symlink_metadata(&current)
        .with_context(|| format!("reading source path {}", current.display()))?;
    Ok((current, metadata.file_type().is_symlink()))
}

fn open_regular(root: &Dir, relative: &Path) -> Result<(cap_std::fs::File, PathBuf)> {
    let mut options = OpenOptions::new();
    options.read(true);
    #[cfg(unix)]
    {
        options.custom_flags(rustix::fs::OFlags::NOFOLLOW.bits() as i32);
    }
    let file = root
        .open_with(relative, &options)
        .with_context(|| format!("opening source file {}", relative.display()))?;
    Ok((file, relative.to_path_buf()))
}

fn verify_manifest_file(root: &Dir, relative: &str, expected: &str) -> Result<Vec<u8>> {
    let rel = safe_relative(relative)?;
    let (target_path, target_hash) = if let Some(description) = expected.strip_prefix("symlink:") {
        let (target, target_hash) = description
            .rsplit_once('|')
            .context("BUILD-RECORDED symlink has no target hash")?;
        let (resolved, is_symlink) = resolve_source_path(root, rel)?;
        if !is_symlink {
            bail!("source entry is not the recorded symlink: {relative}");
        }
        let parent = resolved.parent().unwrap_or_else(|| Path::new(""));
        let parent_dir = if parent.as_os_str().is_empty() {
            root.try_clone()?
        } else {
            root.open_dir(parent)?
        };
        let link = parent_dir.read_link(Path::new(resolved.file_name().unwrap()))?;
        let target_path = lexical_target(parent, &link)?;
        if link != Path::new(target) {
            bail!("source symlink target does not match provenance: {relative}")
        }
        (resolve_directory_path(root, &target_path)?, target_hash)
    } else {
        let parent = resolve_directory_path(root, rel.parent().unwrap_or_else(|| Path::new("")))?;
        (
            parent.join(rel.file_name().context("source path has no filename")?),
            expected,
        )
    };
    let (mut file, resolved) = open_regular(root, &target_path)?;
    let mut bytes = Vec::new();
    file.read_to_end(&mut bytes)
        .with_context(|| format!("reading source file {}", resolved.display()))?;
    if digest(&bytes) != target_hash {
        bail!(
            "source file hash does not match provenance: {}",
            resolved.display()
        )
    }
    if expected.starts_with("symlink:") && bytes.is_empty() {
        bail!("source symlink target is empty: {}", resolved.display())
    }
    Ok(bytes)
}

fn verify_file(root: &Dir, relative: &str, expected: &str) -> Result<Vec<u8>> {
    verify_manifest_file(root, relative, expected)
}

pub(crate) fn read(
    root: &Dir,
    expected: &SourceDefinition,
) -> Result<(Provenance, BTreeMap<String, Vec<u8>>)> {
    let mut file = root
        .open_with(PROVENANCE, &{
            let mut options = OpenOptions::new();
            options.read(true);
            #[cfg(unix)]
            {
                options.custom_flags(rustix::fs::OFlags::NOFOLLOW.bits() as i32);
            }
            options
        })
        .context("reading provenance")?;
    let mut source = String::new();
    file.read_to_string(&mut source)
        .context("reading provenance")?;
    let provenance: Provenance =
        serde_json::from_str(&source).context("parsing source provenance")?;
    if provenance.source_url != expected.source_url
        || provenance.expected_commit != expected.source_commit
        || provenance.source_commit != expected.source_commit
    {
        bail!(
            "BUILD-RECORDED commit does not match selected source commit {}",
            expected.source_commit
        )
    }
    if provenance.files.is_empty() {
        bail!("BUILD-RECORDED manifest has no file hashes")
    }
    let mut files = BTreeMap::new();
    for (path, hash) in &provenance.files {
        files.insert(path.clone(), verify_file(root, path, hash)?);
    }
    if !files.contains_key(&expected.entrypoint) {
        bail!(
            "BUILD-RECORDED manifest has no hash for {}",
            expected.entrypoint
        )
    }
    Ok((provenance, files))
}

#[cfg(test)]
mod tests {
    use std::fs;

    use anyhow::Result;
    use cap_std::{ambient_authority, fs::Dir};

    use super::{digest, verify_file};

    #[cfg(unix)]
    #[test]
    fn verifies_symlink_manifest_entries_by_target_bytes() -> Result<()> {
        let root = tempfile::tempdir()?;
        let cap = Dir::open_ambient_dir(root.path(), ambient_authority())?;
        let target = root.path().join("actual.yaml");
        let link = root.path().join("fedora-standard.yaml");
        fs::write(&target, "image: fedora\n")?;
        std::os::unix::fs::symlink("actual.yaml", &link)?;
        let expected = format!("symlink:actual.yaml|{}", digest(b"image: fedora\n"));
        assert_eq!(
            verify_file(&cap, "fedora-standard.yaml", &expected)?,
            b"image: fedora\n"
        );
        fs::write(target, "tampered\n")?;
        assert!(verify_file(&cap, "fedora-standard.yaml", &expected).is_err());
        Ok(())
    }

    #[cfg(unix)]
    #[test]
    fn accepts_nested_in_tree_symlink_parents() -> Result<()> {
        let root = tempfile::tempdir()?;
        fs::create_dir(root.path().join("real"))?;
        fs::write(root.path().join("real/file"), b"inside")?;
        std::os::unix::fs::symlink("real", root.path().join("nested"))?;
        let cap = Dir::open_ambient_dir(root.path(), ambient_authority())?;
        assert_eq!(
            verify_file(&cap, "nested/file", &digest(b"inside"))?,
            b"inside"
        );
        Ok(())
    }

    #[cfg(unix)]
    #[test]
    fn rejects_symlink_chains_parent_escapes_and_loops() -> Result<()> {
        use std::os::unix::fs::symlink;

        let root = tempfile::tempdir()?;
        let cap = Dir::open_ambient_dir(root.path(), ambient_authority())?;
        let outside = tempfile::tempdir()?;
        fs::write(outside.path().join("file"), b"outside")?;
        symlink(outside.path(), root.path().join("parent"))?;
        assert!(verify_file(&cap, "parent/file", &digest(b"outside")).is_err());

        symlink("chain", root.path().join("entry"))?;
        symlink(outside.path().join("file"), root.path().join("chain"))?;
        assert!(
            verify_file(
                &cap,
                "entry",
                &format!("symlink:chain|{}", digest(b"outside"))
            )
            .is_err()
        );

        symlink("loop-b", root.path().join("loop-a"))?;
        symlink("loop-a", root.path().join("loop-b"))?;
        assert!(
            verify_file(
                &cap,
                "loop-a",
                "symlink:loop-b|0000000000000000000000000000000000000000000000000000000000000000"
            )
            .is_err()
        );
        Ok(())
    }
}
