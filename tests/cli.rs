use assert_cmd::Command;
use predicates::str::contains;
use sha2::{Digest, Sha256};
use std::fs;
use tempfile::tempdir;

const KIWI_COMMIT: &str = "daf359394913068f4fc18a8b7aafc404caeb8257";
const PUNGI_COMMIT: &str = "233933230bb6a97017e937efea20fb9c4d40f948";
const KIWI_URL: &str = "https://forge.fedoraproject.org/releng/kiwi-descriptions";
const PUNGI_URL: &str = "https://forge.fedoraproject.org/releng/pungi-fedora";

fn hash(value: &str) -> String {
    format!("{:x}", Sha256::digest(value.as_bytes()))
}
fn provenance(dir: &std::path::Path, url: &str, commit: &str, files: &[(&str, &str)]) {
    let entries = files
        .iter()
        .map(|(name, contents)| format!("\"{name}\":\"{}\"", hash(contents)))
        .collect::<Vec<_>>()
        .join(",");
    fs::write(dir.join(".source-provenance.json"), format!(r#"{{"source_url":"{url}","expected_commit":"{commit}","source_commit":"{commit}","files":{{{entries}}}}}"#)).unwrap();
    for (name, contents) in files {
        fs::write(dir.join(name), contents).unwrap();
    }
}
fn inspect_args(kiwi: &std::path::Path, pungi: &std::path::Path) -> Vec<String> {
    vec![
        "source".into(),
        "inspect".into(),
        "--from=quay.io/fedora/fedora:45".into(),
        "--source-dir".into(),
        kiwi.display().to_string(),
        "--source-dir".into(),
        pungi.display().to_string(),
    ]
}

#[test]
fn plan_is_rootless_and_records_immutable_commits() {
    for (image, entrypoint) in [
        ("quay.io/fedora/fedora:45", "Fedora.kiwi"),
        ("quay.io/fedora/fedora-bootc:43", "fedora-standard.yaml"),
        ("quay.io/fedora/fedora-bootc:44", "fedora-standard.yaml"),
        ("quay.io/fedora/fedora-bootc:45", "fedora-standard.yaml"),
        ("quay.io/fedora/fedora-silverblue:44", "silverblue.yaml"),
        ("quay.io/fedora/fedora-kinoite:44", "kinoite.yaml"),
        ("quay.io/fedora/fedora-silverblue:45", "silverblue.yaml"),
        ("quay.io/fedora/fedora-kinoite:45", "kinoite.yaml"),
    ] {
        Command::cargo_bin("container-rootfs-builder")
            .unwrap()
            .args(["--from", image, "--plan", "/target-rootfs"])
            .assert()
            .success()
            .stdout(contains("source_commit"))
            .stdout(contains(entrypoint));
    }
}

#[test]
fn non_plan_execution_is_refused() {
    Command::cargo_bin("container-rootfs-builder")
        .unwrap()
        .args(["--from=quay.io/fedora/fedora-bootc:45", "/target-rootfs"])
        .assert()
        .failure()
        .stderr(contains("plan-only"));
}
#[test]
fn aliases_are_rejected() {
    for image in [
        "minimal-bootable",
        "silverblue",
        "quay.io/fedora/fedora:latest",
    ] {
        Command::cargo_bin("container-rootfs-builder")
            .unwrap()
            .args(["--from", image, "--plan", "/target-rootfs"])
            .assert()
            .failure()
            .stderr(contains("unrecognized image reference"));
    }
}

#[cfg(unix)]
#[test]
fn build_rootfs_uses_the_release_manifest_with_a_fake_runner() {
    use std::os::unix::fs::PermissionsExt;

    let root = tempdir().unwrap();
    let source_root = root.path().join("repos");
    fs::create_dir_all(source_root.join("etc")).unwrap();
    fs::write(
        source_root.join("etc/os-release"),
        "ID=fedora\nVERSION_ID=44\n",
    )
    .unwrap();
    let output = root.path().join("runner-args");
    let runner = root.path().join("fake-runner");
    fs::write(
        &runner,
        "#!/bin/sh\nprintf '%s\\n' \"$@\" > \"$RUNNER_ARGS\"\n",
    )
    .unwrap();
    fs::set_permissions(&runner, fs::Permissions::from_mode(0o755)).unwrap();

    for release in ["43", "44"] {
        fs::write(
            source_root.join("etc/os-release"),
            format!("ID=fedora\nVERSION_ID={release}\n"),
        )
        .unwrap();
        let target = root.path().join(format!("rootfs-{release}"));
        Command::cargo_bin("container-rootfs-builder")
            .unwrap()
            .env("CONTAINER_ROOTFS_BUILDER_HELPER", &runner)
            .env("RUNNER_ARGS", &output)
            .args([
                "build-rootfs",
                "--from",
                &format!("quay.io/fedora/fedora-bootc:{release}"),
                "--target",
                target.to_str().unwrap(),
                "--source-root",
                source_root.to_str().unwrap(),
                "--source-root-rw",
            ])
            .assert()
            .success();
        let args = fs::read_to_string(&output).unwrap();
        assert!(args.contains("--manifest=standard"));
        assert!(args.contains(source_root.to_str().unwrap()));
        assert!(args.contains(target.to_str().unwrap()));
        assert!(!args.contains("--target"));
        assert!(!args.contains("releasever"));
    }
}

#[cfg(unix)]
#[test]
fn build_rootfs_dispatches_atomic_desktops_to_native_rpm_ostree() {
    use std::os::unix::fs::PermissionsExt;

    let root = tempdir().unwrap();
    let source_root = root.path().join("repos");
    fs::create_dir_all(source_root.join("etc")).unwrap();
    let output = root.path().join("runner-args");
    let runner = root.path().join("fake-rpm-ostree");
    fs::write(
        &runner,
        "#!/bin/sh\nprintf '%s\\n' \"$@\" >> \"$RUNNER_ARGS\"\nprintf '%s\\n' -- >> \"$RUNNER_ARGS\"\nif [ \"$2\" = tree ]; then printf 'flattened manifest\\n'; fi\n",
    )
    .unwrap();
    fs::set_permissions(&runner, fs::Permissions::from_mode(0o755)).unwrap();
    for (release, variant) in [
        ("44", "silverblue"),
        ("44", "kinoite"),
        ("45", "silverblue"),
        ("45", "kinoite"),
    ] {
        fs::write(
            source_root.join("etc/os-release"),
            format!("ID=fedora\nVERSION_ID={release}\n"),
        )
        .unwrap();
        fs::write(&output, "").unwrap();
        let target = root.path().join(format!("rootfs-{variant}-{release}"));
        Command::cargo_bin("container-rootfs-builder")
            .unwrap()
            .env("CONTAINER_ROOTFS_BUILDER_RPM_OSTREE", &runner)
            .env("RUNNER_ARGS", &output)
            .args([
                "build-rootfs",
                &format!("--from=quay.io/fedora/fedora-{variant}:{release}"),
                "--target",
                target.to_str().unwrap(),
                "--source-root",
                source_root.to_str().unwrap(),
                "--source-root-rw",
            ])
            .assert()
            .success()
            .stdout("");
        let invocations = fs::read_to_string(&output).unwrap();
        let expected = format!(
            "compose\ntree\n--print-only\n--source-root={}\n/sources/f{release}/atomic/{variant}.yaml\n--\ncompose\nrootfs\n--source-root-rw={}\n/sources/f{release}/atomic/{variant}.yaml\n{}\n--\n",
            source_root.display(),
            source_root.display(),
            target.display()
        );
        assert_eq!(invocations, expected);
        assert!(!invocations.contains("bootc-base-imagectl"));
        assert!(!invocations.contains("container lint"));
    }
}

#[cfg(unix)]
#[test]
fn silverblue_stops_when_print_only_validation_fails() {
    use std::os::unix::fs::PermissionsExt;

    let root = tempdir().unwrap();
    let source_root = root.path().join("repos");
    fs::create_dir_all(source_root.join("etc")).unwrap();
    fs::write(
        source_root.join("etc/os-release"),
        "ID=fedora\nVERSION_ID=45\n",
    )
    .unwrap();
    let runner = root.path().join("failing-rpm-ostree");
    fs::write(&runner, "#!/bin/sh\nexit 42\n").unwrap();
    fs::set_permissions(&runner, fs::Permissions::from_mode(0o755)).unwrap();
    Command::cargo_bin("container-rootfs-builder")
        .unwrap()
        .env("CONTAINER_ROOTFS_BUILDER_RPM_OSTREE", &runner)
        .args([
            "build-rootfs",
            "--from=quay.io/fedora/fedora-silverblue:45",
            "--target",
            root.path().join("rootfs").to_str().unwrap(),
            "--source-root",
            source_root.to_str().unwrap(),
            "--source-root-rw",
        ])
        .assert()
        .failure()
        .stderr(contains("compose tree --print-only failed"));
}

#[cfg(unix)]
#[test]
fn silverblue_reports_native_rootfs_failure_after_validation() {
    use std::os::unix::fs::PermissionsExt;

    let root = tempdir().unwrap();
    let source_root = root.path().join("repos");
    fs::create_dir_all(source_root.join("etc")).unwrap();
    fs::write(
        source_root.join("etc/os-release"),
        "ID=fedora\nVERSION_ID=45\n",
    )
    .unwrap();
    let runner = root.path().join("rootfs-failing-rpm-ostree");
    fs::write(
        &runner,
        "#!/bin/sh\nif [ \"$2\" = rootfs ]; then exit 42; fi\nexit 0\n",
    )
    .unwrap();
    fs::set_permissions(&runner, fs::Permissions::from_mode(0o755)).unwrap();
    Command::cargo_bin("container-rootfs-builder")
        .unwrap()
        .env("CONTAINER_ROOTFS_BUILDER_RPM_OSTREE", &runner)
        .args([
            "build-rootfs",
            "--from=quay.io/fedora/fedora-silverblue:45",
            "--target",
            root.path().join("rootfs").to_str().unwrap(),
            "--source-root",
            source_root.to_str().unwrap(),
            "--source-root-rw",
        ])
        .assert()
        .failure()
        .stderr(contains("compose rootfs failed"));
}

#[test]
fn build_rootfs_rejects_source_release_mismatch_and_overlap() {
    let root = tempdir().unwrap();
    let source = root.path().join("repos");
    fs::create_dir_all(source.join("etc")).unwrap();
    fs::write(source.join("etc/os-release"), "ID=fedora\nVERSION_ID=43\n").unwrap();
    let target = root.path().join("target");
    Command::cargo_bin("container-rootfs-builder")
        .unwrap()
        .args([
            "build-rootfs",
            "--from=quay.io/fedora/fedora-bootc:44",
            "--target",
            target.to_str().unwrap(),
            "--source-root",
            source.to_str().unwrap(),
            "--source-root-rw",
        ])
        .assert()
        .failure()
        .stderr(contains(
            "source root is Fedora 43, but --from requests Fedora 44",
        ));

    for (release, variant) in [
        ("44", "silverblue"),
        ("44", "kinoite"),
        ("45", "silverblue"),
        ("45", "kinoite"),
    ] {
        Command::cargo_bin("container-rootfs-builder")
            .unwrap()
            .args([
                "build-rootfs",
                &format!("--from=quay.io/fedora/fedora-{variant}:{release}"),
                "--target",
                root.path()
                    .join(format!("{variant}-{release}-target"))
                    .to_str()
                    .unwrap(),
                "--source-root",
                source.to_str().unwrap(),
                "--source-root-rw",
            ])
            .assert()
            .failure()
            .stderr(contains(format!(
                "source root is Fedora 43, but --from requests Fedora {release}"
            )));
    }

    let nested = source.join("nested");
    Command::cargo_bin("container-rootfs-builder")
        .unwrap()
        .args([
            "build-rootfs",
            "--from=quay.io/fedora/fedora-bootc:43",
            "--target",
            nested.to_str().unwrap(),
            "--source-root",
            source.to_str().unwrap(),
            "--source-root-rw",
        ])
        .assert()
        .failure()
        .stderr(contains("source/target overlap"));
}

#[test]
fn build_rootfs_requires_explicit_source_root_rw() {
    let root = tempdir().unwrap();
    let source = root.path().join("repos");
    fs::create_dir_all(source.join("etc")).unwrap();
    fs::write(source.join("etc/os-release"), "ID=fedora\nVERSION_ID=44\n").unwrap();
    let target = root.path().join("target");
    Command::cargo_bin("container-rootfs-builder")
        .unwrap()
        .args([
            "build-rootfs",
            "--from=quay.io/fedora/fedora-bootc:44",
            "--target",
            target.to_str().unwrap(),
            "--source-root",
            source.to_str().unwrap(),
        ])
        .assert()
        .failure()
        .stderr(contains("pass --source-root-rw"));
}

#[test]
fn repo_image_example_selects_external_repo_files() {
    let file =
        fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/Containerfile.repos")).unwrap();
    assert!(file.contains("FROM quay.io/fedora/fedora:44"));
    assert!(file.contains("rm -f /etc/yum.repos.d/*.repo"));
    assert!(file.contains("COPY repos/*.repo /etc/yum.repos.d/"));
}

#[test]
fn atomic_rootfs_containerfile_does_not_claim_bootability() {
    let file = fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/Containerfile.atomic-rootfs"
    ))
    .unwrap();
    assert!(file.contains("ARG FEDORA_RELEASE=44"));
    assert!(file.contains("ARG ATOMIC_VARIANT=silverblue"));
    assert!(file.contains("quay.io/fedora/fedora-${ATOMIC_VARIANT}:${FEDORA_RELEASE}"));
    assert!(file.contains(
        "quay.io/fedora/fedora@sha256:8938dce2600de0b78f5ef8d1541192f207fdafb7414d83957f6687147aa8998b"
    ));
    assert!(file.contains("--source-root=/repos --source-root-rw"));
    assert!(!file.contains("LABEL"));
    assert!(!file.contains("bootc container lint"));
}

#[test]
fn atomic_desktop_catalog_has_exact_native_inputs() {
    for (release, variant, commit) in [
        (
            "44",
            "silverblue",
            "1a1effa1ae6ef22c961ff5962ec314d9208231e1",
        ),
        ("44", "kinoite", "1a1effa1ae6ef22c961ff5962ec314d9208231e1"),
        (
            "45",
            "silverblue",
            "9dbdbe2f1c8009b2257201ffd4719100ff0b0ba1",
        ),
        ("45", "kinoite", "9dbdbe2f1c8009b2257201ffd4719100ff0b0ba1"),
    ] {
        let output = Command::cargo_bin("container-rootfs-builder")
            .unwrap()
            .args([
                "--from",
                &format!("quay.io/fedora/fedora-{variant}:{release}"),
                "--plan",
                "/target-rootfs",
            ])
            .output()
            .unwrap();
        assert!(output.status.success());
        let plan: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
        assert_eq!(plan["declared_release"], release);
        assert_eq!(plan["engine"]["kind"], "rpm-ostree-rootfs");
        assert_eq!(
            plan["source_definitions"][0]["entrypoint"],
            format!("{variant}.yaml")
        );
        assert_eq!(
            plan["source_definitions"][0]["source_ref"],
            format!("f{release}")
        );
        assert_eq!(plan["source_definitions"][0]["source_commit"], commit);
        assert_eq!(plan["source_definitions"][0]["release_tag"], release);
        assert_eq!(
            plan["source_definitions"][0]["observed_source_release"],
            release
        );
        assert_eq!(plan["output_contract"]["artifact"], "filesystem-rootfs");
        assert_eq!(plan["output_contract"]["bootability"], "not-asserted");
    }
}

#[test]
fn build_rootfs_rejects_unknown_and_existing_targets() {
    let root = tempdir().unwrap();
    let existing = root.path().join("existing");
    fs::create_dir(&existing).unwrap();
    Command::cargo_bin("container-rootfs-builder")
        .unwrap()
        .args([
            "build-rootfs",
            "--from=quay.io/fedora/fedora:45",
            "--target",
            existing.to_str().unwrap(),
            "--source-root=/",
        ])
        .assert()
        .failure()
        .stderr(contains("remains plan-only"));
    Command::cargo_bin("container-rootfs-builder")
        .unwrap()
        .args([
            "build-rootfs",
            "--from=quay.io/fedora/fedora-bootc:43",
            "--target",
            existing.to_str().unwrap(),
            "--source-root=/",
        ])
        .assert()
        .failure()
        .stderr(contains("refusing to overwrite"));

    let target = root.path().join("new-target");
    Command::cargo_bin("container-rootfs-builder")
        .unwrap()
        .args([
            "build-rootfs",
            "--from=quay.io/fedora/fedora-bootc:43",
            "--target",
            target.to_str().unwrap(),
            "--source-root=/does-not-exist",
        ])
        .assert()
        .failure()
        .stderr(contains(
            "source root must be an existing absolute directory",
        ));
}

fn fixtures(
    profile: bool,
    commented_pungi: bool,
    unsafe_include: bool,
    triple_quoted_pungi: bool,
) -> (tempfile::TempDir, std::path::PathBuf, Vec<String>) {
    let root = tempdir().unwrap();
    let kiwi = root.path().join("kiwi");
    let pungi = root.path().join("pungi");
    fs::create_dir_all(kiwi.join("teams/cloud")).unwrap();
    fs::create_dir_all(&pungi).unwrap();
    let included = if profile {
        "<image><profile name=\"Container-Base-Generic\"/></image>"
    } else {
        "<image><profile name=\"other\"/></image>"
    };
    let top = if unsafe_include {
        "<image name=\"Fedora\"><include from=\"this://./../escape.xml\"/><preferences><release-version>45</release-version></preferences></image>"
    } else {
        "<image name=\"Fedora\"><include from=\"this://./teams/cloud/container.xml\"/><preferences><release-version>45</release-version></preferences></image>"
    };
    let pungi_text = if triple_quoted_pungi {
        "\"\"\"\nrelease_version = '45'\nkiwibuild_description_path = 'Fedora.kiwi'\n'kiwi_profile': 'Container-Base-Generic',\n\"\"\"\n"
    } else if commented_pungi {
        "# release_version = '45'\n# kiwibuild_description_path = 'Fedora.kiwi'\n# 'kiwi_profile': 'Container-Base-Generic',\n"
    } else {
        "release_version = '45'\nkiwibuild_description_path = 'Fedora.kiwi'\n'kiwi_profile': 'Container-Base-Generic',\n"
    };
    provenance(
        &kiwi,
        KIWI_URL,
        KIWI_COMMIT,
        &[
            ("Fedora.kiwi", top),
            ("teams/cloud/container.xml", included),
        ],
    );
    provenance(
        &pungi,
        PUNGI_URL,
        PUNGI_COMMIT,
        &[("fedora-container.conf", pungi_text)],
    );
    (root, kiwi.clone(), inspect_args(&kiwi, &pungi))
}

#[test]
fn inspect_accepts_xml_includes_and_static_settings() {
    let (_root, _kiwi, args) = fixtures(true, false, false, false);
    Command::cargo_bin("container-rootfs-builder")
        .unwrap()
        .args(args)
        .assert()
        .success()
        .stdout(contains("source-inspect-only"))
        .stdout(contains(KIWI_COMMIT));
}
#[test]
fn inspect_rejects_wrong_commit() {
    let (_root, kiwi, mut args) = fixtures(true, false, false, false);
    fs::write(kiwi.join(".source-provenance.json"), format!(r#"{{"source_url":"{KIWI_URL}","expected_commit":"deadbeef","source_commit":"deadbeef","files":{{"Fedora.kiwi":"{}","teams/cloud/container.xml":"{}"}}}}"#, hash("bad"), hash("bad"))).unwrap();
    Command::cargo_bin("container-rootfs-builder")
        .unwrap()
        .args(args.drain(..))
        .assert()
        .failure()
        .stderr(contains("BUILD-RECORDED commit"));
}

#[cfg(unix)]
#[test]
fn inspect_rejects_provenance_symlink_escape() {
    use std::os::unix::fs::symlink;
    let (_root, kiwi, args) = fixtures(true, false, false, false);
    let outside = tempdir().unwrap();
    fs::write(outside.path().join("provenance"), b"{}").unwrap();
    fs::remove_file(kiwi.join(".source-provenance.json")).unwrap();
    symlink(
        outside.path().join("provenance"),
        kiwi.join(".source-provenance.json"),
    )
    .unwrap();
    Command::cargo_bin("container-rootfs-builder")
        .unwrap()
        .args(args)
        .assert()
        .failure()
        .stderr(contains("reading provenance"));
}
#[test]
fn inspect_rejects_missing_profile() {
    let (_root, _kiwi, args) = fixtures(false, false, false, false);
    Command::cargo_bin("container-rootfs-builder")
        .unwrap()
        .args(args)
        .assert()
        .failure()
        .stderr(contains("do not define profile"));
}
#[test]
fn inspect_rejects_commented_pungi_settings() {
    let (_root, _kiwi, args) = fixtures(true, true, false, false);
    Command::cargo_bin("container-rootfs-builder")
        .unwrap()
        .args(args)
        .assert()
        .failure()
        .stderr(contains("non-commented"));
}

#[test]
fn inspect_rejects_tampered_included_xml() {
    let (_root, kiwi, args) = fixtures(true, false, false, false);
    fs::write(
        kiwi.join("teams/cloud/container.xml"),
        "<image><profile name=\"Container-Base-Generic\"/></image><tampered/>",
    )
    .unwrap();
    Command::cargo_bin("container-rootfs-builder")
        .unwrap()
        .args(args)
        .assert()
        .failure()
        .stderr(contains("hash does not match"));
}

#[test]
fn inspect_rejects_unsafe_include() {
    let (_root, _kiwi, args) = fixtures(true, false, true, false);
    Command::cargo_bin("container-rootfs-builder")
        .unwrap()
        .args(args)
        .assert()
        .failure()
        .stderr(contains("unsafe source path"));
}

#[test]
fn inspect_rejects_include_present_but_missing_from_manifest() {
    let (_root, kiwi, args) = fixtures(true, false, false, false);
    let manifest_path = kiwi.join(".source-provenance.json");
    let manifest = fs::read_to_string(&manifest_path).unwrap();
    let included_hash = hash("<image><profile name=\"Container-Base-Generic\"/></image>");
    fs::write(
        &manifest_path,
        manifest.replace(
            &format!(",\"teams/cloud/container.xml\":\"{included_hash}\""),
            "",
        ),
    )
    .unwrap();
    Command::cargo_bin("container-rootfs-builder")
        .unwrap()
        .args(args)
        .assert()
        .failure()
        .stderr(contains("absent from BUILD-RECORDED manifest"));
}

#[test]
fn inspect_rejects_expected_settings_inside_triple_quoted_text() {
    let (_root, _kiwi, args) = fixtures(true, false, false, true);
    Command::cargo_bin("container-rootfs-builder")
        .unwrap()
        .args(args)
        .assert()
        .failure()
        .stderr(contains("triple-quoted"));
}
