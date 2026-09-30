//! Opt-in container integration tests for supported Fedora root filesystems.

use std::{
    env,
    fs::{self, File},
    path::{Path, PathBuf},
    process::{Command, ExitStatus, Stdio},
};

use serde_json::Value;

const REPOS_IMAGE: &str =
    "quay.io/fedora/fedora@sha256:80d49c6c7c4303efb5eebc0317e343588d5d482146ca2be48eb82494d2a83060";
const RUNTIME_IMAGE: &str =
    "quay.io/fedora/fedora@sha256:e1e716b2f6ca98e1ed41e62d32a7bcbf1637d5b931894299eab9c659a16fe8ed";
const BUILDER_IMAGE: &str = "localhost/container-rootfs-builder:integration";

#[derive(Clone, Copy, Debug)]
struct Case {
    name: &'static str,
    variant: Option<&'static str>,
    containerfile: &'static str,
}

const CASES: &[Case] = &[
    Case {
        name: "bootc44",
        variant: None,
        containerfile: "Containerfile.rootfs",
    },
    Case {
        name: "silverblue44",
        variant: Some("silverblue"),
        containerfile: "Containerfile.atomic-rootfs",
    },
    Case {
        name: "kinoite44",
        variant: Some("kinoite"),
        containerfile: "Containerfile.atomic-rootfs",
    },
];

fn select_cases(selection: &str) -> Result<Vec<Case>, String> {
    if selection.trim() == "all" {
        return Ok(CASES.to_vec());
    }
    let cases = selection
        .split(',')
        .map(str::trim)
        .map(|name| {
            if name.is_empty() {
                return Err("integration case selection contains an empty case".to_owned());
            }
            CASES
                .iter()
                .copied()
                .find(|case| case.name == name)
                .ok_or_else(|| format!("unknown integration case {name:?}; choose all, bootc44, silverblue44, or kinoite44"))
        })
        .collect::<Result<Vec<_>, _>>()?;
    if cases.is_empty() {
        return Err("integration case selection is empty".to_owned());
    }
    Ok(cases)
}

fn selected_cases() -> Vec<Case> {
    let selection = env::var("INTEGRATION_CASES").unwrap_or_else(|_| "all".into());
    select_cases(&selection).unwrap_or_else(|error| panic!("{error}"))
}

fn log_dir() -> PathBuf {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("target/integration-logs");
    fs::create_dir_all(&dir).expect("create integration log directory");
    dir
}

fn command_with_args(program: &str, args: &[&str], log: &Path) -> ExitStatus {
    let stdout =
        File::create(log).unwrap_or_else(|error| panic!("create {}: {error}", log.display()));
    let stderr_log = log.with_extension("stderr.log");
    let stderr = File::create(&stderr_log)
        .unwrap_or_else(|error| panic!("create {}: {error}", stderr_log.display()));
    Command::new(program)
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .args(args)
        .stdout(Stdio::from(stdout))
        .stderr(Stdio::from(stderr))
        .status()
        .unwrap_or_else(|error| panic!("run {program}: {error}; log {}", log.display()))
}

fn checked(program: &str, args: &[&str], name: &str, logs: &Path) {
    let log = logs.join(format!("{name}.log"));
    let stderr_log = log.with_extension("stderr.log");
    let status = command_with_args(program, args, &log);
    if !status.success() {
        panic!(
            "FAIL {name} ({status}; stdout {}; stderr {})",
            log.display(),
            stderr_log.display()
        );
    }
    eprintln!("PASS {name}");
}

fn output_text(name: &str, logs: &Path) -> String {
    let log = logs.join(format!("{name}.log"));
    let text = fs::read_to_string(&log)
        .unwrap_or_else(|error| panic!("FAIL {name}: read {}: {error}", log.display()))
        .trim()
        .to_owned();
    if text.is_empty() {
        panic!(
            "FAIL {name}: command produced no stdout (see {})",
            log.display()
        );
    }
    text
}

fn podman_run(logs: &Path, name: &str, image: &str, args: &[&str]) {
    let (entrypoint, command_args) = args.split_first().expect("entrypoint");
    let mut full = vec!["run", "--rm", "--entrypoint", *entrypoint, image];
    full.extend_from_slice(command_args);
    checked("podman", &full, name, logs)
}

fn image_inspect(logs: &Path, name: &str, image: &str) -> Value {
    checked(
        "podman",
        &["image", "inspect", "--format", "json", image],
        name,
        logs,
    );
    let log = logs.join(format!("{name}.log"));
    let text = fs::read_to_string(&log)
        .unwrap_or_else(|error| panic!("FAIL {name}: read {}: {error}", log.display()));
    serde_json::from_str(&text)
        .unwrap_or_else(|error| panic!("FAIL {name}: invalid inspect JSON: {error}"))
}

#[test]
#[ignore = "requires Fedora Linux, Podman, network access, /dev/fuse, and substantial disk/memory"]
fn container_images_have_expected_rootfs_contracts() {
    let logs = log_dir();
    let cases = selected_cases();
    // Do not let a locally cached image hide a removed or mistyped Quay pin.
    for (name, image) in [
        ("remote-repos-image", REPOS_IMAGE),
        ("remote-runtime-image", RUNTIME_IMAGE),
    ] {
        let reference = format!("docker://{image}");
        checked("skopeo", &["inspect", &reference], name, &logs);
    }
    checked(
        "podman",
        &[
            "build",
            "--security-opt=label=disable",
            "--cap-add=all",
            "--device=/dev/fuse",
            "-t",
            BUILDER_IMAGE,
            ".",
        ],
        "builder",
        &logs,
    );

    // Exercise the helper's default embedded source catalog without rebuilding
    // or mounting any external repository content.
    checked(
        "podman",
        &[
            "run",
            "--rm",
            BUILDER_IMAGE,
            "source",
            "inspect",
            "--from",
            "quay.io/fedora/fedora:45",
            "--source-dir",
            "/sources/f45/kiwi",
            "--source-dir",
            "/sources/f45/pungi",
        ],
        "source-inspect",
        &logs,
    );

    // Keep this smoke test small and network-free; it also covers the host
    // image-diff wrapper and its identical-image result.
    checked(
        env!("CARGO_BIN_EXE_image-diff"),
        &[
            "--helper-image",
            BUILDER_IMAGE,
            BUILDER_IMAGE,
            BUILDER_IMAGE,
        ],
        "image-diff-identical",
        &logs,
    );

    for case in cases {
        let tag = format!("localhost/container-rootfs-integration-{}:44", case.name);
        let repos_arg = format!("REPOS_IMAGE={REPOS_IMAGE}");
        let mut build_args = vec![
            "build".to_owned(),
            "--security-opt=label=disable".to_owned(),
            "--cap-add=all".to_owned(),
            "--device=/dev/fuse".to_owned(),
            "--build-arg".to_owned(),
            "CONFIG_BUILDER=localhost/container-rootfs-builder:integration".to_owned(),
            "--build-arg".to_owned(),
            repos_arg,
        ];
        if let Some(variant) = case.variant {
            build_args.extend([
                "--build-arg".to_owned(),
                format!("ATOMIC_VARIANT={variant}"),
            ]);
        }
        build_args.extend([
            "--build-arg".to_owned(),
            "FEDORA_RELEASE=44".to_owned(),
            "-f".to_owned(),
            case.containerfile.to_owned(),
            "-t".to_owned(),
        ]);
        build_args.push(tag.clone());
        build_args.push(".".to_owned());
        let build_refs = build_args.iter().map(String::as_str).collect::<Vec<_>>();
        checked(
            "podman",
            &build_refs,
            &format!("build-{}", case.name),
            &logs,
        );

        podman_run(
            &logs,
            &format!("release-{}", case.name),
            &tag,
            &["rpm", "-E", "%{fedora}"],
        );
        assert_eq!(output_text(&format!("release-{}", case.name), &logs), "44");
        podman_run(
            &logs,
            &format!("kernel-{}", case.name),
            &tag,
            &[
                "sh",
                "-c",
                "set -eu; test -n \"$(find /usr/lib/modules -type f -name 'vmlinuz*' -print -quit)\"; test -n \"$(find /usr/lib/modules -type f -name 'initramfs*' -print -quit)\"",
            ],
        );
        if let Some(variant) = case.variant {
            let package = format!("fedora-release-{variant}");
            podman_run(
                &logs,
                &format!("variant-release-{}", case.name),
                &tag,
                &["rpm", "-q", &package],
            );
            let inspect = image_inspect(&logs, &format!("inspect-{}", case.name), &tag);
            let config = &inspect[0]["Config"];
            assert!(
                config["Labels"].get("containers.bootc").is_none(),
                "{} unexpectedly has bootc label",
                case.name
            );
            assert!(
                config["Labels"].get("ostree.bootable").is_none(),
                "{} unexpectedly has ostree label",
                case.name
            );
            assert!(
                config["Cmd"].is_null() || config["Cmd"].as_array().is_some_and(Vec::is_empty),
                "{} unexpectedly has Cmd",
                case.name
            );
            assert!(
                config["Entrypoint"].is_null()
                    || config["Entrypoint"].as_array().is_some_and(Vec::is_empty),
                "{} unexpectedly has entrypoint",
                case.name
            );
        } else {
            checked(
                "podman",
                &[
                    "run",
                    "--rm",
                    "--security-opt=label=disable",
                    "--cap-add=all",
                    "--device=/dev/fuse",
                    "--entrypoint",
                    "bootc",
                    &tag,
                    "container",
                    "lint",
                ],
                "bootc-lint-44",
                &logs,
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::select_cases;

    #[test]
    fn case_selection_accepts_all_and_subsets() {
        assert_eq!(select_cases("all").unwrap().len(), 3);
        let selected = select_cases("kinoite44,bootc44").unwrap();
        assert_eq!(
            selected.iter().map(|case| case.name).collect::<Vec<_>>(),
            ["kinoite44", "bootc44"]
        );
    }

    #[test]
    fn case_selection_rejects_empty_and_unknown_cases() {
        assert!(select_cases("").is_err());
        assert!(select_cases("bootc44,").is_err());
        assert!(select_cases("unknown").is_err());
    }

    #[test]
    fn source_checkout_retries_only_the_pinned_fetch() {
        let containerfile =
            std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/Containerfile"))
                .expect("read Containerfile");
        assert!(containerfile.contains("RUN set -euo pipefail;"));
        assert!(
            containerfile.contains("while ! git -C \"$work\" fetch --depth=1 origin \"$expected\"")
        );
        assert!(containerfile.contains("if [ \"$attempt\" -ge 3 ]; then return 1; fi"));
        assert!(containerfile.contains("sleep \"$((attempt * 10 - 5))\""));
        assert!(containerfile.contains("git -C \"$work\" rev-parse HEAD"));
        assert!(containerfile.contains("test \"$actual\" = \"$expected\""));
    }
}
