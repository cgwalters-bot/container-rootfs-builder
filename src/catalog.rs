//! Supported image catalog and stable planning output contracts.

use std::path::Path;

use anyhow::{Context, Result};
use serde::Serialize;

const KIWI_DESCRIPTIONS: &str = "https://forge.fedoraproject.org/releng/kiwi-descriptions";
const PUNGI_FEDORA: &str = "https://forge.fedoraproject.org/releng/pungi-fedora";
const BOOTC_BASE_IMAGES: &str = "https://gitlab.com/fedora/bootc/base-images";
const ATOMIC_DESKTOPS_CONFIG: &str = "https://forge.fedoraproject.org/atomic-desktops/config";
const KIWI_COMMIT: &str = "daf359394913068f4fc18a8b7aafc404caeb8257";
const PUNGI_COMMIT: &str = "233933230bb6a97017e937efea20fb9c4d40f948";
const BOOTC_COMMIT: &str = "bbea58e7db3b403785d632d7db3f75bfe6cd5415";
const F44_ATOMIC_COMMIT: &str = "1a1effa1ae6ef22c961ff5962ec314d9208231e1";
const F45_ATOMIC_COMMIT: &str = "9dbdbe2f1c8009b2257201ffd4719100ff0b0ba1";

#[derive(Clone, Copy, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum DescriptionFormat {
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
pub(crate) struct SourceDefinition {
    pub(crate) source_url: String,
    source_ref: String,
    pub(crate) source_commit: String,
    pub(crate) entrypoint: String,
    description_format: DescriptionFormat,
    selected_profile: String,
    release_tag: String,
    observed_source_release: Option<String>,
    verification_status: VerificationStatus,
}

#[derive(Clone, Copy, Debug, Serialize, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub(crate) enum BuildEngine {
    PlanOnly,
    BootcBaseImagectl { manifest: &'static str },
    RpmOstreeRootfs,
}

#[derive(Clone, Copy, Debug, Serialize, PartialEq, Eq)]
struct OutputContract {
    artifact: &'static str,
    preserves_ostree_commit: bool,
    includes_native_oci_config: bool,
    bootability: &'static str,
}

#[derive(Debug, Serialize)]
pub(crate) struct Plan<'a> {
    image_reference: &'a str,
    declared_release: &'static str,
    target: &'a Path,
    execution: &'static str,
    engine: BuildEngine,
    output_contract: OutputContract,
    source_definitions: Vec<SourceDefinition>,
    recipe_packages: Vec<String>,
    note: &'static str,
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct SourceTemplate {
    source_url: &'static str,
    source_ref: &'static str,
    source_commit: &'static str,
    pub(crate) checkout_root: &'static str,
    pub(crate) entrypoint: &'static str,
    description_format: DescriptionFormat,
    selected_profile: &'static str,
    observed_source_release: Option<&'static str>,
}

#[derive(Clone, Copy, Debug)]
pub(crate) enum CatalogSources {
    FedoraContainer,
    One(SourceTemplate),
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct CatalogEntry {
    image_reference: &'static str,
    pub(crate) declared_release: &'static str,
    pub(crate) sources: CatalogSources,
    pub(crate) engine: BuildEngine,
}

const BOOTC_SOURCE: SourceTemplate = SourceTemplate {
    source_url: BOOTC_BASE_IMAGES,
    source_ref: "main",
    source_commit: BOOTC_COMMIT,
    checkout_root: "/sources/bootc",
    entrypoint: "fedora-standard.yaml",
    description_format: DescriptionFormat::BootcBaseImagesManifest,
    selected_profile: "fedora-standard",
    // The standard manifest is release-neutral; the mounted repository root
    // selects the release and is checked immediately before execution.
    observed_source_release: None,
};

const fn atomic_source(
    source_ref: &'static str,
    commit: &'static str,
    checkout_root: &'static str,
    observed_release: &'static str,
    variant: &'static str,
    manifest: &'static str,
) -> SourceTemplate {
    SourceTemplate {
        source_url: ATOMIC_DESKTOPS_CONFIG,
        source_ref,
        source_commit: commit,
        checkout_root,
        entrypoint: manifest,
        description_format: DescriptionFormat::AtomicDesktopsConfig,
        selected_profile: variant,
        observed_source_release: Some(observed_release),
    }
}

const CATALOG: &[CatalogEntry] = &[
    CatalogEntry {
        image_reference: "quay.io/fedora/fedora:45",
        declared_release: "45",
        sources: CatalogSources::FedoraContainer,
        engine: BuildEngine::PlanOnly,
    },
    CatalogEntry {
        image_reference: "quay.io/fedora/fedora-bootc:43",
        declared_release: "43",
        sources: CatalogSources::One(BOOTC_SOURCE),
        engine: BuildEngine::BootcBaseImagectl {
            manifest: "standard",
        },
    },
    CatalogEntry {
        image_reference: "quay.io/fedora/fedora-bootc:44",
        declared_release: "44",
        sources: CatalogSources::One(BOOTC_SOURCE),
        engine: BuildEngine::BootcBaseImagectl {
            manifest: "standard",
        },
    },
    CatalogEntry {
        image_reference: "quay.io/fedora/fedora-bootc:45",
        declared_release: "45",
        sources: CatalogSources::One(BOOTC_SOURCE),
        engine: BuildEngine::BootcBaseImagectl {
            manifest: "standard",
        },
    },
    CatalogEntry {
        image_reference: "quay.io/fedora/fedora-silverblue:44",
        declared_release: "44",
        sources: CatalogSources::One(atomic_source(
            "f44",
            F44_ATOMIC_COMMIT,
            "/sources/f44/atomic",
            "44",
            "silverblue",
            "silverblue.yaml",
        )),
        engine: BuildEngine::RpmOstreeRootfs,
    },
    CatalogEntry {
        image_reference: "quay.io/fedora/fedora-kinoite:44",
        declared_release: "44",
        sources: CatalogSources::One(atomic_source(
            "f44",
            F44_ATOMIC_COMMIT,
            "/sources/f44/atomic",
            "44",
            "kinoite",
            "kinoite.yaml",
        )),
        engine: BuildEngine::RpmOstreeRootfs,
    },
    CatalogEntry {
        image_reference: "quay.io/fedora/fedora-silverblue:45",
        declared_release: "45",
        sources: CatalogSources::One(atomic_source(
            "f45",
            F45_ATOMIC_COMMIT,
            "/sources/f45/atomic",
            "45",
            "silverblue",
            "silverblue.yaml",
        )),
        engine: BuildEngine::RpmOstreeRootfs,
    },
    CatalogEntry {
        image_reference: "quay.io/fedora/fedora-kinoite:45",
        declared_release: "45",
        sources: CatalogSources::One(atomic_source(
            "f45",
            F45_ATOMIC_COMMIT,
            "/sources/f45/atomic",
            "45",
            "kinoite",
            "kinoite.yaml",
        )),
        engine: BuildEngine::RpmOstreeRootfs,
    },
];

pub(crate) fn entry(image: &str) -> Result<&'static CatalogEntry> {
    CATALOG
        .iter()
        .find(|entry| entry.image_reference == image)
        .with_context(|| {
            let accepted = CATALOG
                .iter()
                .map(|entry| entry.image_reference)
                .collect::<Vec<_>>()
                .join(", ");
            format!("unrecognized image reference {image:?}; use one of: {accepted}")
        })
}

fn output_contract(engine: BuildEngine) -> OutputContract {
    match engine {
        BuildEngine::PlanOnly => OutputContract {
            artifact: "none-plan-only",
            preserves_ostree_commit: false,
            includes_native_oci_config: false,
            bootability: "not-applicable",
        },
        BuildEngine::BootcBaseImagectl { .. } | BuildEngine::RpmOstreeRootfs => OutputContract {
            artifact: "filesystem-rootfs",
            preserves_ostree_commit: false,
            includes_native_oci_config: false,
            bootability: "not-asserted",
        },
    }
}

fn execution_name(engine: BuildEngine) -> &'static str {
    match engine {
        BuildEngine::PlanOnly => "plan-only",
        BuildEngine::BootcBaseImagectl { .. } => "bootc-base-imagectl",
        BuildEngine::RpmOstreeRootfs => "rpm-ostree-compose-rootfs",
    }
}

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
        observed_source_release: None,
        verification_status: VerificationStatus::CandidateNotRecordedProvenance,
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

fn definition_from_template(template: SourceTemplate, release: &str) -> SourceDefinition {
    SourceDefinition {
        source_url: template.source_url.into(),
        source_ref: template.source_ref.into(),
        source_commit: template.source_commit.into(),
        entrypoint: template.entrypoint.into(),
        description_format: template.description_format,
        selected_profile: template.selected_profile.into(),
        release_tag: release.into(),
        observed_source_release: template.observed_source_release.map(String::from),
        verification_status: VerificationStatus::CandidateNotRecordedProvenance,
    }
}

pub(crate) fn source_definitions(image: &str) -> Result<Vec<SourceDefinition>> {
    let entry = entry(image)?;
    Ok(match entry.sources {
        CatalogSources::FedoraContainer => vec![
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
        CatalogSources::One(source) => {
            vec![definition_from_template(source, entry.declared_release)]
        }
    })
}

pub(crate) fn plan<'a>(
    image: &'a str,
    target: &'a Path,
    recipe_packages: Vec<String>,
) -> Result<Plan<'a>> {
    let entry = entry(image)?;
    Ok(Plan {
        image_reference: image,
        declared_release: entry.declared_release,
        target,
        execution: execution_name(entry.engine),
        engine: entry.engine,
        output_contract: output_contract(entry.engine),
        source_definitions: source_definitions(image)?,
        recipe_packages,
        note: "Source refs and immutable expected source commits are recorded. This candidate mapping is not published-image provenance; no rootfs is built.",
    })
}

#[cfg(test)]
mod tests {
    use anyhow::Result;

    use super::source_definitions;

    #[test]
    fn maps_supported_images() -> Result<()> {
        assert_eq!(source_definitions("quay.io/fedora/fedora:45")?.len(), 2);
        for release in ["43", "44", "45"] {
            let definition =
                &source_definitions(&format!("quay.io/fedora/fedora-bootc:{release}"))?[0];
            assert_eq!(definition.source_ref, "main");
            assert_eq!(definition.release_tag, release);
            assert_eq!(definition.observed_source_release, None);
        }
        for image in [
            "minimal-bootable",
            "silverblue",
            "quay.io/fedora/fedora:latest",
        ] {
            assert!(source_definitions(image).is_err())
        }
        Ok(())
    }
}
