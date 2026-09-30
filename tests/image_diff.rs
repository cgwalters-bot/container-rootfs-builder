use assert_cmd::Command;
use predicates::prelude::PredicateBooleanExt;
use std::fs;
use tempfile::tempdir;

fn run(first: &std::path::Path, second: &std::path::Path) -> assert_cmd::assert::Assert {
    Command::cargo_bin("image-diff")
        .unwrap()
        .args(["compare", first.to_str().unwrap(), second.to_str().unwrap()])
        .assert()
}

#[test]
fn compares_the_core_tree_cases() {
    let root = tempdir().unwrap();
    let first = root.path().join("first");
    let second = root.path().join("second");
    fs::create_dir_all(&first).unwrap();
    fs::create_dir_all(&second).unwrap();

    run(&first, &second).code(0);
    fs::write(first.join("content"), b"one").unwrap();
    run(&first, &second)
        .code(1)
        .stdout(predicates::str::contains("- content"));
    fs::write(second.join("content"), b"two").unwrap();
    run(&first, &second)
        .code(1)
        .stdout(predicates::str::contains("~ content"));
    fs::remove_file(first.join("content")).unwrap();
    run(&first, &second)
        .code(1)
        .stdout(predicates::str::contains("+ content"));

    fs::remove_file(second.join("content")).unwrap();
    #[cfg(unix)]
    std::os::unix::fs::symlink("target", first.join("link")).unwrap();
    #[cfg(unix)]
    {
        std::os::unix::fs::symlink("other", second.join("link")).unwrap();
        run(&first, &second)
            .code(1)
            .stdout(predicates::str::contains("link target"));
    }
}

#[cfg(unix)]
#[test]
fn reports_permissions_and_type_changes() {
    use std::os::unix::fs::PermissionsExt;

    let root = tempdir().unwrap();
    let first = root.path().join("first");
    let second = root.path().join("second");
    fs::create_dir_all(first.join("directory")).unwrap();
    fs::create_dir_all(second.join("directory")).unwrap();
    fs::write(first.join("item"), b"same").unwrap();
    fs::write(second.join("item"), b"same").unwrap();
    fs::set_permissions(first.join("item"), fs::Permissions::from_mode(0o600)).unwrap();
    fs::set_permissions(second.join("item"), fs::Permissions::from_mode(0o644)).unwrap();
    run(&first, &second)
        .code(1)
        .stdout(predicates::str::contains("permissions"));
    fs::remove_file(first.join("item")).unwrap();
    fs::create_dir(first.join("item")).unwrap();
    run(&first, &second)
        .code(1)
        .stdout(predicates::str::contains("file type"));
}

#[test]
fn missing_root_is_an_error() {
    let root = tempdir().unwrap();
    let first = root.path().join("missing");
    let second = root.path().join("second");
    fs::create_dir(&second).unwrap();
    run(&first, &second)
        .code(2)
        .stderr(predicates::str::contains("opening first root"));
}

#[test]
fn relative_roots_use_the_current_directory() {
    let root = tempdir().unwrap();
    fs::create_dir(root.path().join("first")).unwrap();
    fs::create_dir(root.path().join("second")).unwrap();
    Command::cargo_bin("image-diff")
        .unwrap()
        .current_dir(root.path())
        .args(["compare", "first", "second"])
        .assert()
        .code(0)
        .stdout("identical\n");
}

#[test]
fn host_rejects_mount_option_injection() {
    for reference in ["good:one,two", "-bad", " bad", "bad "] {
        let mut command = Command::cargo_bin("image-diff").unwrap();
        if reference.starts_with('-') {
            command.arg("--");
        }
        command
            .args([reference, "good:two"])
            .assert()
            .code(2)
            .stderr(predicates::str::contains("invalid image reference"));
    }
}

#[cfg(unix)]
#[test]
fn escapes_unusual_filenames_without_forging_output_lines() {
    let root = tempdir().unwrap();
    let first = root.path().join("first");
    let second = root.path().join("second");
    fs::create_dir(&first).unwrap();
    fs::create_dir(&second).unwrap();
    fs::write(second.join("line\nname"), b"content").unwrap();

    run(&first, &second)
        .code(1)
        .stdout(predicates::str::contains("+ line\\nname (added)"))
        .stdout(predicates::str::contains("+ line\nname (added)").not());
}
