//! KIWI description and manifest-covered include validation.

use std::{
    collections::{BTreeMap, HashSet},
    path::PathBuf,
};

use anyhow::{Context, Result, bail};

use crate::catalog::SourceDefinition;
use crate::provenance::safe_relative;

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

fn included_files(
    relative: &str,
    seen: &mut HashSet<PathBuf>,
    files: &BTreeMap<String, Vec<u8>>,
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
    for node in doc
        .descendants()
        .filter(|node| node.has_tag_name("include"))
    {
        let value = node
            .attribute("from")
            .or_else(|| node.attribute("file"))
            .or_else(|| node.attribute("name"))
            .or_else(|| node.attribute("path"))
            .context("KIWI include has no file, name, or path")?;
        included_files(value, seen, files, included)?;
    }
    Ok(())
}

pub(crate) fn validate(
    definition: &SourceDefinition,
    files: &BTreeMap<String, Vec<u8>>,
) -> Result<()> {
    let source =
        std::str::from_utf8(&files[&definition.entrypoint]).context("KIWI XML is not UTF-8")?;
    let doc = roxmltree::Document::parse(source).context("parsing KIWI XML")?;
    let root = doc.root_element();
    if root.tag_name().name() != "image"
        || root.attribute("name") != Some("Fedora")
        || !doc.descendants().any(|node| {
            node.has_tag_name("release-version")
                && node.text().is_some_and(|value| value.trim() == "45")
        })
    {
        bail!("KIWI entrypoint is not the Fedora 45 image description")
    }
    let mut seen = HashSet::new();
    let mut included = Vec::new();
    included_files(
        &format!("this://./{}", definition.entrypoint),
        &mut seen,
        files,
        &mut included,
    )?;
    let profile = included.iter().any(|path| {
        std::str::from_utf8(&files[path]).is_ok_and(|source| {
            roxmltree::Document::parse(source).is_ok_and(|document| {
                document.descendants().any(|node| {
                    node.has_tag_name("profile")
                        && node.attribute("name") == Some("Container-Base-Generic")
                })
            })
        })
    });
    if !profile {
        bail!("included KIWI files do not define profile Container-Base-Generic")
    }
    Ok(())
}
