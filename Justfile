default: list

list:
    @just --list

fmt:
    @mkdir -p target/integration-logs
    @cargo fmt --check >target/integration-logs/cargo-fmt.log 2>&1 && echo 'PASS fmt (target/integration-logs/cargo-fmt.log)' || { echo 'FAIL fmt (target/integration-logs/cargo-fmt.log)'; exit 1; }

check:
    @mkdir -p target/integration-logs
    @cargo check --locked >target/integration-logs/cargo-check.log 2>&1 && echo 'PASS check (target/integration-logs/cargo-check.log)' || { echo 'FAIL check (target/integration-logs/cargo-check.log)'; exit 1; }

test:
    @mkdir -p target/integration-logs
    @cargo test --locked >target/integration-logs/cargo-test.log 2>&1 && echo 'PASS test (target/integration-logs/cargo-test.log)' || { echo 'FAIL test (target/integration-logs/cargo-test.log)'; exit 1; }

clippy:
    @mkdir -p target/integration-logs
    @cargo clippy --all-targets --locked -- -D warnings >target/integration-logs/cargo-clippy.log 2>&1 && echo 'PASS clippy (target/integration-logs/cargo-clippy.log)' || { echo 'FAIL clippy (target/integration-logs/cargo-clippy.log)'; exit 1; }

check-all: fmt check test clippy

# Build the helper image without changing the normal :latest tag.
builder:
    @mkdir -p target/integration-logs
    @podman build --security-opt=label=disable --cap-add=all --device=/dev/fuse -t localhost/container-rootfs-builder:integration . >target/integration-logs/builder-manual.log 2>&1 && echo 'PASS builder (target/integration-logs/builder-manual.log)' || { echo 'FAIL builder (target/integration-logs/builder-manual.log)'; exit 1; }

# Full builds are deliberately opt-in. CASE may be all, bootc44, silverblue44,
# or kinoite44 (or a comma-separated selection).
integration case='all':
    @mkdir -p target/integration-logs
    @INTEGRATION_CASES={{quote(case)}} cargo test --test container_integration --locked -- --ignored --nocapture >target/integration-logs/integration-suite.log 2>&1 && echo 'PASS integration (target/integration-logs/integration-suite.log)' || { echo 'FAIL integration (target/integration-logs/integration-suite.log)'; exit 1; }
