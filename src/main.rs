#![forbid(unsafe_code)]

use std::{
    collections::HashSet,
    io::Read,
    path::{Component, Path, PathBuf},
    process::Command as ProcessCommand,
};

use anyhow::{Context, Result, bail};
use cap_std::{
    ambient_authority,
    fs::{Dir, OpenOptions, OpenOptionsExt},
};
use clap::Parser;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

#[derive(Parser, Debug)]
#[command(about = "Plan source-backed Fedora container rootfs work", version)]
struct Cli {
    #[command(subcommand)]
    command: Option<Command>,
    #[arg(long)]
    from: Option<String>,
    #[arg(long)]
    recipe: Option<PathBuf>,
    #[arg(long)]
    plan: bool,
    target_rootfs: Option<PathBuf>,
}
#[derive(clap::Subcommand, Debug)]
enum Command {
    #[command(subcommand)]
    Source(SourceCommand),
    /// Build one of the explicitly supported Fedora bootc root filesystems.
    BuildRootfs(BuildRootfsArgs),
}
#[derive(clap::Subcommand, Debug)]
enum SourceCommand {
    Inspect(InspectArgs),
}
#[derive(clap::Args, Debug)]
struct InspectArgs {
    #[arg(long)]
    from: String,
    #[arg(long = "source-dir", required = true)]
    source_dirs: Vec<PathBuf>,
}
#[derive(clap::Args, Debug)]
struct BuildRootfsArgs {
    /// Exact supported Fedora bootc OCI reference.
    #[arg(long)]
    from: String,
    /// A new, nonexistent directory to receive the root filesystem.
    #[arg(long)]
    target: PathBuf,
    /// Mounted Fedora repository image root used by rpm-ostree for DNF configuration.
    #[arg(long)]
    source_root: PathBuf,
    /// Explicitly allow the helper to mutate the repository source root.
    #[arg(long)]
    source_root_rw: bool,
    /// Permit an explicitly requested cross-release build (advanced use).
    #[arg(long)]
    allow_cross_release: bool,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Blueprint {
    #[serde(default)]
    packages: Vec<Package>,
    customizations: Option<Customizations>,
}
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Package {
    name: String,
    version: Option<String>,
}
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Customizations {
    sshkey: Option<Vec<serde::de::IgnoredAny>>,
}
#[derive(Clone, Copy, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
enum DescriptionFormat {
    Kiwi,
    PungiConfig,
    BootcBaseImagesManifest,
    AtomicDesktopsConfig,
}
#[derive(Clone, Copy, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
enum VerificationStatus {
    CandidateNotRecordedProvenance,
}
#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
struct SourceDefinition {
    source_url: String,
    source_ref: String,
    source_commit: String,
    entrypoint: String,
    description_format: DescriptionFormat,
    selected_profile: String,
    release_tag: String,
    verification_status: VerificationStatus,
    build_supported: bool,
}
#[derive(Debug, Serialize)]
struct Plan<'a> {
    image_reference: &'a str,
    target: &'a Path,
    execution: &'static str,
    source_definitions: Vec<SourceDefinition>,
    recipe_packages: Vec<String>,
    note: &'static str,
}
#[derive(Debug, Serialize)]
struct Inspection<'a> {
    image_reference: &'a str,
    source_definitions: Vec<SourceDefinition>,
    validated_entrypoints: Vec<String>,
    execution: &'static str,
    note: &'static str,
}

const KIWI_DESCRIPTIONS: &str = "https://forge.fedoraproject.org/releng/kiwi-descriptions";
const PUNGI_FEDORA: &str = "https://forge.fedoraproject.org/releng/pungi-fedora";
const BOOTC_BASE_IMAGES: &str = "https://gitlab.com/fedora/bootc/base-images";
const ATOMIC_DESKTOPS_CONFIG: &str = "https://forge.fedoraproject.org/atomic-desktops/config";
const KIWI_COMMIT: &str = "daf359394913068f4fc18a8b7aafc404caeb8257";
const PUNGI_COMMIT: &str = "233933230bb6a97017e937efea20fb9c4d40f948";
const BOOTC_COMMIT: &str = "bbea58e7db3b403785d632d7db3f75bfe6cd5415";
const ATOMIC_COMMIT: &str = "9dbdbe2f1c8009b2257201ffd4719100ff0b0ba1";
const PROVENANCE: &str = ".source-provenance.json";
const BOOTC_HELPER: &str = "/usr/local/bin/bootc-base-imagectl";

fn definition_for_release(
    url: &str,
    reference: &str,
    commit: &str,
    entrypoint: &str,
    format: DescriptionFormat,
    profile: &str,
    release: &str,
) -> SourceDefinition {
    SourceDefinition {
        source_url: url.into(),
        source_ref: reference.into(),
        source_commit: commit.into(),
        entrypoint: entrypoint.into(),
        description_format: format,
        selected_profile: profile.into(),
        release_tag: release.into(),
        verification_status: VerificationStatus::CandidateNotRecordedProvenance,
        build_supported: false,
    }
}
fn definition(
    url: &str,
    reference: &str,
    commit: &str,
    entrypoint: &str,
    format: DescriptionFormat,
    profile: &str,
) -> SourceDefinition {
    definition_for_release(url, reference, commit, entrypoint, format, profile, "45")
}
fn bootc_definition(release: &str) -> SourceDefinition {
    let mut definition = definition_for_release(
        BOOTC_BASE_IMAGES,
        &format!("f{release}"),
        BOOTC_COMMIT,
        "fedora-standard.yaml",
        DescriptionFormat::BootcBaseImagesManifest,
        "fedora-standard",
        release,
    );
    definition.build_supported = true;
    definition
}
fn source_definitions(image: &str) -> Result<Vec<SourceDefinition>> {
    Ok(match image {
        "quay.io/fedora/fedora:45" => vec![
            definition(
                KIWI_DESCRIPTIONS,
                "f45",
                KIWI_COMMIT,
                "Fedora.kiwi",
                DescriptionFormat::Kiwi,
                "Container-Base-Generic",
            ),
            definition(
                PUNGI_FEDORA,
                "f45",
                PUNGI_COMMIT,
                "fedora-container.conf",
                DescriptionFormat::PungiConfig,
                "Container-Base-Generic",
            ),
        ],
        "quay.io/fedora/fedora-bootc:43" => vec![bootc_definition("43")],
        "quay.io/fedora/fedora-bootc:44" => vec![bootc_definition("44")],
        "quay.io/fedora/fedora-bootc:45" => vec![bootc_definition("45")],
        "quay.io/fedora/fedora-silverblue:45" => vec![definition(
            ATOMIC_DESKTOPS_CONFIG,
            "f45",
            ATOMIC_COMMIT,
            "silverblue.yaml",
            DescriptionFormat::AtomicDesktopsConfig,
            "silverblue",
        )],
        _ => bail!(
            "unrecognized image reference {image:?}; use one of: quay.io/fedora/fedora:45, quay.io/fedora/fedora-bootc:<43|44|45>, quay.io/fedora/fedora-silverblue:45"
        ),
    })
}

fn bootc_release(image: &str) -> Result<&str> {
    match image {
        "quay.io/fedora/fedora-bootc:43" => Ok("43"),
        "quay.io/fedora/fedora-bootc:44" => Ok("44"),
        "quay.io/fedora/fedora-bootc:45" => Ok("45"),
        _ => bail!(
            "build-rootfs supports only quay.io/fedora/fedora-bootc:<43|44|45>; other references remain plan-only"
        ),
    }
}

fn helper_path() -> PathBuf {
    std::env::var_os("CONTAINER_ROOTFS_BUILDER_HELPER")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(BOOTC_HELPER))
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

fn source_release(source_root: &Path) -> Result<String> {
    let os_release = std::fs::read_to_string(source_root.join("etc/os-release"))
        .with_context(|| format!("reading {}/etc/os-release", source_root.display()))?;
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

fn build_rootfs(args: BuildRootfsArgs) -> Result<()> {
    let requested_release = bootc_release(&args.from)?;
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
            "source root is read-only by default; pass --source-root-rw only for a disposable repository source root because the helper may mutate it"
        );
    }
    let helper = helper_path();
    let status = ProcessCommand::new(&helper)
        .arg("build-rootfs")
        .arg("--manifest=standard")
        .arg(&args.source_root)
        .arg(&args.target)
        .status()
        .with_context(|| format!("running embedded bootc helper {}", helper.display()))?;
    if !status.success() {
        bail!("embedded bootc helper failed with {status}");
    }
    Ok(())
}
fn read_recipe(path: &Path) -> Result<Vec<String>> {
    let source = std::fs::read_to_string(path)
        .with_context(|| format!("reading recipe {}", path.display()))?;
    let blueprint: Blueprint = match path.extension().and_then(|x| x.to_str()) {
        Some("toml") => toml::from_str(&source).context("parsing TOML blueprint")?,
        Some("ncl") => nickel_lang_core::deserialize::from_str(&source)
            .map_err(|e| anyhow::anyhow!("evaluating Nickel recipe: {e}"))?,
        _ => bail!("recipe must have a .toml or .ncl extension"),
    };
    if blueprint.customizations.is_some_and(|x| x.sshkey.is_some()) {
        bail!(
            "blueprint customizations.sshkey is unsupported: source-backed planning cannot safely apply it"
        )
    }
    blueprint
        .packages
        .into_iter()
        .map(|p| {
            valid_package(&p.name)?;
            let value = match p.version {
                Some(version) => {
                    valid_package(&version)?;
                    format!("{}-{version}", p.name)
                }
                None => p.name,
            };
            Ok(value)
        })
        .collect()
}
fn valid_package(value: &str) -> Result<()> {
    if value.is_empty()
        || !value.bytes().all(|c| {
            c.is_ascii_alphanumeric() || matches!(c, b'.' | b'_' | b'+' | b'-' | b':' | b'~')
        })
    {
        bail!("invalid package or version {value:?}")
    }
    Ok(())
}

#[derive(Debug, Deserialize)]
struct Provenance {
    source_url: String,
    expected_commit: String,
    source_commit: String,
    files: std::collections::BTreeMap<String, String>,
}
fn safe_relative(path: &str) -> Result<&Path> {
    let p = Path::new(path);
    if p.is_absolute()
        || p.components().any(|c| {
            matches!(
                c,
                Component::ParentDir | Component::RootDir | Component::Prefix(_)
            )
        })
    {
        bail!("unsafe source path {path:?}")
    }
    Ok(p)
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
        options.custom_flags(libc::O_NOFOLLOW);
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
fn provenance(
    root: &Dir,
    expected: &SourceDefinition,
) -> Result<(Provenance, std::collections::BTreeMap<String, Vec<u8>>)> {
    let mut file = root
        .open_with(PROVENANCE, &{
            let mut o = OpenOptions::new();
            o.read(true);
            #[cfg(unix)]
            {
                o.custom_flags(libc::O_NOFOLLOW);
            }
            o
        })
        .context("reading provenance")?;
    let mut source = String::new();
    file.read_to_string(&mut source)
        .context("reading provenance")?;
    let p: Provenance = serde_json::from_str(&source).context("parsing source provenance")?;
    if p.source_url != expected.source_url
        || p.expected_commit != expected.source_commit
        || p.source_commit != expected.source_commit
    {
        bail!(
            "BUILD-RECORDED commit does not match selected source commit {}",
            expected.source_commit
        )
    }
    if p.files.is_empty() {
        bail!("BUILD-RECORDED manifest has no file hashes")
    }
    let mut files = std::collections::BTreeMap::new();
    for (path, hash) in &p.files {
        files.insert(path.clone(), verify_file(root, path, hash)?);
    }
    if !files.contains_key(&expected.entrypoint) {
        bail!(
            "BUILD-RECORDED manifest has no hash for {}",
            expected.entrypoint
        )
    }
    Ok((p, files))
}

fn include_path(value: &str) -> Result<String> {
    let value = value
        .strip_prefix("this://")
        .context("KIWI include must use the this:// scheme")?;
    let value = value
        .strip_prefix("./")
        .context("KIWI this:// include must be relative to the source root")?;
    safe_relative(value)?;
    Ok(value.to_string())
}
fn kiwi_files(
    relative: &str,
    seen: &mut HashSet<PathBuf>,
    files: &std::collections::BTreeMap<String, Vec<u8>>,
    included: &mut Vec<String>,
) -> Result<()> {
    let relative = include_path(relative)?;
    let identity = PathBuf::from(&relative);
    if !seen.insert(identity) {
        bail!("cyclic or repeated KIWI include: {relative}")
    }
    let source = files.get(&relative).with_context(|| {
        format!("KIWI include is absent from BUILD-RECORDED manifest: {relative}")
    })?;
    let source = std::str::from_utf8(source).context("KIWI XML is not UTF-8")?;
    let doc = roxmltree::Document::parse(source)
        .with_context(|| format!("parsing KIWI XML {relative}"))?;
    included.push(relative);
    for node in doc.descendants().filter(|n| n.has_tag_name("include")) {
        let value = node
            .attribute("from")
            .or_else(|| node.attribute("file"))
            .or_else(|| node.attribute("name"))
            .or_else(|| node.attribute("path"))
            .context("KIWI include has no file, name, or path")?;
        kiwi_files(value, seen, files, included)?;
    }
    Ok(())
}
fn inspect_sources<'a>(image: &'a str, dirs: &[PathBuf]) -> Result<Inspection<'a>> {
    let mut definitions = source_definitions(image)?;
    if image != "quay.io/fedora/fedora:45" {
        bail!("source inspect currently supports only quay.io/fedora/fedora:45")
    }
    if dirs.len() != definitions.len() {
        bail!(
            "source inspect for {image} requires {} --source-dir arguments",
            definitions.len()
        )
    }
    let kiwi_root = Dir::open_ambient_dir(&dirs[0], ambient_authority())?;
    let pungi_root = Dir::open_ambient_dir(&dirs[1], ambient_authority())?;
    let (kiwi_prov, kiwi_files_verified) = provenance(&kiwi_root, &definitions[0])?;
    let source = std::str::from_utf8(&kiwi_files_verified[&definitions[0].entrypoint])
        .context("KIWI XML is not UTF-8")?;
    let doc = roxmltree::Document::parse(source).context("parsing KIWI XML")?;
    let root = doc.root_element();
    if root.tag_name().name() != "image"
        || root.attribute("name") != Some("Fedora")
        || !doc.descendants().any(|n| {
            n.has_tag_name("release-version") && n.text().is_some_and(|x| x.trim() == "45")
        })
    {
        bail!("KIWI entrypoint is not the Fedora 45 image description")
    }
    let mut seen = HashSet::new();
    let mut included = Vec::new();
    kiwi_files(
        &format!("this://./{}", definitions[0].entrypoint),
        &mut seen,
        &kiwi_files_verified,
        &mut included,
    )?;
    let profile = included.iter().any(|p| {
        std::str::from_utf8(&kiwi_files_verified[p]).is_ok_and(|s| {
            roxmltree::Document::parse(s).is_ok_and(|d| {
                d.descendants().any(|n| {
                    n.has_tag_name("profile")
                        && n.attribute("name") == Some("Container-Base-Generic")
                })
            })
        })
    });
    if !profile {
        bail!("included KIWI files do not define profile Container-Base-Generic")
    }
    let (pungi_prov, pungi_files_verified) = provenance(&pungi_root, &definitions[1])?;
    let text = std::str::from_utf8(&pungi_files_verified[&definitions[1].entrypoint])
        .context("Pungi entrypoint is not UTF-8")?;
    if text.contains("'''") || text.contains("\"\"\"") {
        bail!("Pungi literal check rejects triple-quoted strings")
    }
    let assignment = |key: &str, value: &str| -> bool {
        text.lines().any(|line| {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                return false;
            }
            let pattern = format!("{key} = '{value}'");
            line == pattern || line == format!("{key} = \"{value}\"")
        })
    };
    let has_profile = text.lines().any(|line| {
        let line = line.trim();
        line == "'kiwi_profile': 'Container-Base-Generic',"
            || line == "\"kiwi_profile\": \"Container-Base-Generic\","
    });
    if !assignment("release_version", "45")
        || !assignment("kiwibuild_description_path", "Fedora.kiwi")
        || !has_profile
    {
        bail!("Pungi entrypoint lacks a non-commented expected setting")
    }
    if kiwi_prov.source_commit != definitions[0].source_commit {
        bail!("KIWI BUILD-RECORDED commit does not match selected source commit")
    }
    if pungi_prov.source_commit != definitions[1].source_commit {
        bail!("Pungi BUILD-RECORDED commit does not match selected source commit")
    }
    definitions[0].source_commit = kiwi_prov.source_commit;
    definitions[1].source_commit = pungi_prov.source_commit;
    Ok(Inspection {
        image_reference: image,
        validated_entrypoints: vec![
            definitions[0].entrypoint.clone(),
            definitions[1].entrypoint.clone(),
        ],
        source_definitions: definitions,
        execution: "source-inspect-only",
        note: "Compared BUILD-RECORDED commit strings and internal file hashes, parsed manifest-covered XML includes, and checked literal noncommented Pungi settings only. This is not Git provenance or published-image provenance.",
    })
}

fn run(cli: Cli) -> Result<()> {
    match cli.command {
        Some(Command::Source(SourceCommand::Inspect(args))) => {
            println!(
                "{}",
                serde_json::to_string_pretty(&inspect_sources(&args.from, &args.source_dirs)?)?
            );
            return Ok(());
        }
        Some(Command::BuildRootfs(args)) => return build_rootfs(args),
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
        .map(read_recipe)
        .transpose()?
        .unwrap_or_default();
    let plan = Plan {
        image_reference: from,
        target,
        execution: "plan-only",
        source_definitions: source_definitions(from)?,
        recipe_packages: packages,
        note: "Source refs and immutable expected source commits are recorded. This candidate mapping is not published-image provenance; no rootfs is built.",
    };
    if !cli.plan {
        bail!(
            "all recognized image references are plan-only; pass --plan to print the source mapping without building a rootfs"
        )
    }
    println!("{}", serde_json::to_string_pretty(&plan)?);
    Ok(())
}
fn main() {
    if let Err(e) = run(Cli::parse()) {
        eprintln!("container-rootfs-builder: {e:#}");
        std::process::exit(1)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::io::Write;
    use tempfile::NamedTempFile;
    #[test]
    fn maps_and_reads_recipes() -> Result<()> {
        assert_eq!(source_definitions("quay.io/fedora/fedora:45")?.len(), 2);
        for release in ["43", "44", "45"] {
            let definition =
                &source_definitions(&format!("quay.io/fedora/fedora-bootc:{release}"))?[0];
            assert_eq!(definition.source_ref, format!("f{release}"));
            assert_eq!(definition.release_tag, release);
            assert!(definition.build_supported);
        }
        for x in [
            "minimal-bootable",
            "silverblue",
            "quay.io/fedora/fedora:latest",
        ] {
            assert!(source_definitions(x).is_err())
        }
        let mut f = NamedTempFile::with_suffix(".toml")?;
        writeln!(f, "[[packages]]\nname=\"bash\"\nversion=\"5.2\"")?;
        assert_eq!(read_recipe(f.path())?, ["bash-5.2"]);
        writeln!(f, "\n[[packages]]\nname=\"bad\"\nversion=\"?\"")?;
        assert!(read_recipe(f.path()).is_err());
        Ok(())
    }

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
