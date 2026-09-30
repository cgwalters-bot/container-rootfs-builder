#![forbid(unsafe_code)]

use std::{
    collections::BTreeMap,
    env,
    ffi::OsString,
    io::Read,
    path::{Path, PathBuf},
    process::{Command, ExitCode},
};

use anyhow::{Context, Result, bail};
use cap_std::{ambient_authority, fs::Dir};
use clap::Parser;

const DEFAULT_HELPER_IMAGE: &str = "localhost/container-rootfs-builder:latest";

#[derive(Parser, Debug)]
#[command(about = "Compare two Podman image filesystems")]
struct HostArgs {
    #[arg(long, default_value = DEFAULT_HELPER_IMAGE)]
    helper_image: String,
    first: String,
    second: String,
}

fn main() -> ExitCode {
    match run() {
        Ok(code) => ExitCode::from(code),
        Err(error) => {
            eprintln!("image-diff: {error:#}");
            ExitCode::from(2)
        }
    }
}

fn run() -> Result<u8> {
    let mut args = env::args_os();
    let _program = args.next();
    if args.next().as_deref() == Some(std::ffi::OsStr::new("compare")) {
        let first = args.next().context("compare requires FIRST")?;
        let second = args.next().context("compare requires SECOND")?;
        if args.next().is_some() {
            bail!("compare accepts exactly FIRST and SECOND")
        }
        return compare(&PathBuf::from(first), &PathBuf::from(second));
    }

    let cli = HostArgs::parse_from(
        std::iter::once(OsString::from("image-diff")).chain(env::args_os().skip(1)),
    );
    validate_image_ref(&cli.first)?;
    validate_image_ref(&cli.second)?;
    validate_image_ref(&cli.helper_image)?;

    let status = Command::new("podman")
        .arg("run")
        .arg("--rm")
        .arg("--network=none")
        .arg("--mount")
        .arg(format!(
            "type=image,src={},target=/first,rw=false",
            cli.first
        ))
        .arg("--mount")
        .arg(format!(
            "type=image,src={},target=/second,rw=false",
            cli.second
        ))
        .arg("--entrypoint")
        .arg("/usr/local/bin/image-diff")
        .arg(&cli.helper_image)
        .arg("compare")
        .arg("/first")
        .arg("/second")
        .status()
        .context("running podman")?;
    Ok(status.code().filter(|code| *code <= 2).unwrap_or(2) as u8)
}

fn validate_image_ref(reference: &str) -> Result<()> {
    if reference.is_empty()
        || reference.starts_with('-')
        || reference.chars().any(char::is_whitespace)
        || reference.contains(',')
        || reference.chars().any(char::is_control)
    {
        bail!(
            "invalid image reference (empty, option-like, whitespace, comma, or control character): {reference:?}"
        )
    }
    Ok(())
}

fn compare(first: &Path, second: &Path) -> Result<u8> {
    let first = Dir::open_ambient_dir(first, ambient_authority())
        .with_context(|| format!("opening first root {}", first.display()))?;
    let second = Dir::open_ambient_dir(second, ambient_authority())
        .with_context(|| format!("opening second root {}", second.display()))?;
    let mut differences = 0u64;
    compare_dir(&first, &second, Path::new(""), &mut differences)?;
    println!(
        "{}",
        if differences == 0 {
            "identical".to_string()
        } else {
            format!("{differences} difference(s)")
        }
    );
    Ok(if differences == 0 { 0 } else { 1 })
}

fn compare_dir(first: &Dir, second: &Dir, relative: &Path, differences: &mut u64) -> Result<()> {
    let first_entries = entries(first, relative)?;
    let second_entries = entries(second, relative)?;
    for name in first_entries
        .keys()
        .chain(second_entries.keys())
        .cloned()
        .collect::<std::collections::BTreeSet<_>>()
    {
        let path = relative.join(&name);
        match (first_entries.get(&name), second_entries.get(&name)) {
            (Some(_), None) => report(differences, '-', &path, "removed"),
            (None, Some(_)) => report(differences, '+', &path, "added"),
            (Some(first_entry), Some(second_entry)) => {
                compare_entry(
                    first,
                    second,
                    &name,
                    &path,
                    first_entry,
                    second_entry,
                    differences,
                )?;
            }
            (None, None) => unreachable!(),
        }
    }
    Ok(())
}

struct EntryInfo {
    file_type: cap_std::fs::FileType,
    mode: u32,
    kind: u32,
}

fn entries(dir: &Dir, relative: &Path) -> Result<BTreeMap<OsString, EntryInfo>> {
    let mut result = BTreeMap::new();
    let entries = dir
        .entries()
        .with_context(|| format!("reading directory {}", display_path(relative)))?;
    for entry in entries {
        let entry =
            entry.with_context(|| format!("reading directory {}", display_path(relative)))?;
        let file_type = entry.file_type().with_context(|| {
            format!(
                "reading type for {}",
                display_path(&relative.join(entry.file_name()))
            )
        })?;
        let (mode, kind) = if file_type.is_symlink() {
            (0, 0)
        } else {
            use cap_std::fs::MetadataExt;
            let metadata = entry.metadata().with_context(|| {
                format!(
                    "reading metadata for {}",
                    display_path(&relative.join(entry.file_name()))
                )
            })?;
            #[cfg(unix)]
            let kind = metadata.mode() & libc::S_IFMT;
            #[cfg(not(unix))]
            let kind = u32::from(file_type.is_dir()) * 1 + u32::from(file_type.is_file()) * 2;
            (metadata.mode() & 0o7777, kind)
        };
        result.insert(
            entry.file_name(),
            EntryInfo {
                file_type,
                mode,
                kind,
            },
        );
    }
    Ok(result)
}

fn compare_entry(
    first_dir: &Dir,
    second_dir: &Dir,
    name: &OsString,
    path: &Path,
    first: &EntryInfo,
    second: &EntryInfo,
    differences: &mut u64,
) -> Result<()> {
    if first.kind != second.kind
        || first.file_type.is_dir() != second.file_type.is_dir()
        || first.file_type.is_file() != second.file_type.is_file()
        || first.file_type.is_symlink() != second.file_type.is_symlink()
    {
        report(differences, '~', path, "file type");
        return Ok(());
    }
    if first.mode != second.mode {
        report(differences, '~', path, "permissions");
    }
    if first.file_type.is_dir() {
        let first_child = first_dir
            .open_dir(name)
            .with_context(|| format!("opening first directory {}", display_path(path)))?;
        let second_child = second_dir
            .open_dir(name)
            .with_context(|| format!("opening second directory {}", display_path(path)))?;
        compare_dir(&first_child, &second_child, path, differences)?;
    } else if first.file_type.is_symlink() {
        let first_target = first_dir
            .read_link_contents(name)
            .with_context(|| format!("reading first link {}", display_path(path)))?;
        let second_target = second_dir
            .read_link_contents(name)
            .with_context(|| format!("reading second link {}", display_path(path)))?;
        if first_target != second_target {
            report(differences, '~', path, "link target");
        }
    } else if first.file_type.is_file() && !same_bytes(first_dir, second_dir, name, path)? {
        report(differences, '~', path, "contents");
    }
    Ok(())
}

fn same_bytes(first_dir: &Dir, second_dir: &Dir, name: &OsString, path: &Path) -> Result<bool> {
    let mut first = first_dir
        .open(name)
        .with_context(|| format!("opening first regular file {}", display_path(path)))?;
    let mut second = second_dir
        .open(name)
        .with_context(|| format!("opening second regular file {}", display_path(path)))?;
    let mut first_buffer = [0u8; 128 * 1024];
    let mut second_buffer = [0u8; 128 * 1024];
    loop {
        let first_read = read_chunk(&mut first, &mut first_buffer, path, "first")?;
        let second_read = read_chunk(&mut second, &mut second_buffer, path, "second")?;
        if first_read != second_read || first_buffer[..first_read] != second_buffer[..second_read] {
            return Ok(false);
        }
        if first_read == 0 {
            return Ok(true);
        }
    }
}

fn read_chunk(
    file: &mut cap_std::fs::File,
    buffer: &mut [u8],
    path: &Path,
    side: &str,
) -> Result<usize> {
    let mut filled = 0;
    while filled < buffer.len() {
        let read = file
            .read(&mut buffer[filled..])
            .with_context(|| format!("reading {side} regular file {}", display_path(path)))?;
        if read == 0 {
            break;
        }
        filled += read;
    }
    Ok(filled)
}

fn report(count: &mut u64, marker: char, path: &Path, reason: &str) {
    *count += 1;
    println!("{marker} {} ({reason})", display_path(path));
}

fn display_path(path: &Path) -> String {
    #[cfg(unix)]
    {
        use std::os::unix::ffi::OsStrExt;
        let mut displayed = String::new();
        for byte in path.as_os_str().as_bytes() {
            match byte {
                b'\\' => displayed.push_str("\\\\"),
                b'\n' => displayed.push_str("\\n"),
                b'\r' => displayed.push_str("\\r"),
                b'\t' => displayed.push_str("\\t"),
                0x20..=0x7e => displayed.push(*byte as char),
                byte => displayed.push_str(&format!("\\x{byte:02x}")),
            }
        }
        displayed
    }
    #[cfg(not(unix))]
    {
        path.to_string_lossy().into_owned()
    }
}

#[cfg(test)]
mod tests {
    use super::validate_image_ref;

    #[test]
    fn image_reference_validation_is_data_driven() {
        for (reference, valid) in [
            ("fedora:44", true),
            ("localhost/a:b", true),
            ("a,b", false),
            ("-image", false),
            ("image ", false),
            ("", false),
        ] {
            assert_eq!(
                validate_image_ref(reference).is_ok(),
                valid,
                "{reference:?}"
            );
        }
    }
}
