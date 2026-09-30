//! Source checkout inspection and its serialized result.

use std::path::PathBuf;

use anyhow::{Result, bail};
use cap_std::{ambient_authority, fs::Dir};
use serde::Serialize;

use crate::{catalog, kiwi, provenance, pungi};

#[derive(Debug, Serialize)]
pub(crate) struct Inspection<'a> {
    image_reference: &'a str,
    source_definitions: Vec<catalog::SourceDefinition>,
    validated_entrypoints: Vec<String>,
    execution: &'static str,
    note: &'static str,
}

pub(crate) fn inspect<'a>(image: &'a str, dirs: &[PathBuf]) -> Result<Inspection<'a>> {
    let mut definitions = catalog::source_definitions(image)?;
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
    let (kiwi_provenance, kiwi_files) = provenance::read(&kiwi_root, &definitions[0])?;
    kiwi::validate(&definitions[0], &kiwi_files)?;
    let (pungi_provenance, pungi_files) = provenance::read(&pungi_root, &definitions[1])?;
    pungi::validate(&pungi_files[&definitions[1].entrypoint])?;
    if kiwi_provenance.source_commit != definitions[0].source_commit {
        bail!("KIWI BUILD-RECORDED commit does not match selected source commit")
    }
    if pungi_provenance.source_commit != definitions[1].source_commit {
        bail!("Pungi BUILD-RECORDED commit does not match selected source commit")
    }
    definitions[0].source_commit = kiwi_provenance.source_commit;
    definitions[1].source_commit = pungi_provenance.source_commit;
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
