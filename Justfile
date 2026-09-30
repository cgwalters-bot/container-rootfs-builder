set positional-arguments := true

default: list

list:
    @just --list

# Build the native binaries.
build:
    @mkdir -p target/integration-logs
    @cargo build --locked >target/integration-logs/cargo-build.log 2>&1 && echo 'PASS build (target/integration-logs/cargo-build.log)' || { echo 'FAIL build (target/integration-logs/cargo-build.log)'; exit 1; }

# Format Rust sources.
fmt:
    @mkdir -p target/integration-logs
    @cargo fmt >target/integration-logs/cargo-fmt.log 2>&1 && echo 'PASS fmt (target/integration-logs/cargo-fmt.log)' || { echo 'FAIL fmt (target/integration-logs/cargo-fmt.log)'; exit 1; }

# Check Rust formatting without changing files.
fmt-check:
    @mkdir -p target/integration-logs
    @cargo fmt --check >target/integration-logs/cargo-fmt-check.log 2>&1 && echo 'PASS fmt-check (target/integration-logs/cargo-fmt-check.log)' || { echo 'FAIL fmt-check (target/integration-logs/cargo-fmt-check.log)'; exit 1; }

check:
    @mkdir -p target/integration-logs
    @cargo check --all-targets --locked >target/integration-logs/cargo-check.log 2>&1 && echo 'PASS check (target/integration-logs/cargo-check.log)' || { echo 'FAIL check (target/integration-logs/cargo-check.log)'; exit 1; }

# Run unit and ordinary command-line tests; ignored integration tests stay opt-in.
unit *ARGS:
    #!/usr/bin/env bash
    set -euo pipefail
    mkdir -p target/integration-logs
    if command -v cargo-nextest &>/dev/null; then
        test_command=(cargo nextest run --locked "$@")
    else
        test_command=(cargo test --locked "$@")
    fi
    if "${test_command[@]}" >target/integration-logs/cargo-test.log 2>&1; then
        echo 'PASS unit (target/integration-logs/cargo-test.log)'
    else
        echo 'FAIL unit (target/integration-logs/cargo-test.log)'
        exit 1
    fi

# Compatibility alias.

alias test := unit

clippy:
    @mkdir -p target/integration-logs
    @cargo clippy --all-targets --locked -- -D warnings >target/integration-logs/cargo-clippy.log 2>&1 && echo 'PASS clippy (target/integration-logs/cargo-clippy.log)' || { echo 'FAIL clippy (target/integration-logs/cargo-clippy.log)'; exit 1; }

# Static checks.
validate: fmt-check check clippy

check-all: validate unit

# Provision the repository-local documentation environment.
docs-venv:
    @mkdir -p target/integration-logs target/docs-venv
    @if [[ ! -x target/docs-venv/bin/zensical ]]; then \
        python3 -m venv target/docs-venv && \
        target/docs-venv/bin/python -m pip install --disable-pip-version-check -r requirements-docs.txt >target/integration-logs/docs-venv-install.log 2>&1; \
    fi

# Build the documentation in a repository-local Python environment.
docs: docs-venv
    @target/docs-venv/bin/zensical build --clean

# Build the documentation with strict link and configuration checks.
docs-check: docs-venv
    #!/usr/bin/env bash
    set -euo pipefail
    if target/docs-venv/bin/zensical build --clean --strict >target/integration-logs/zensical.log 2>&1; then
        echo 'PASS docs-check (target/integration-logs/zensical.log)'
    else
        echo 'FAIL docs-check (target/integration-logs/zensical.log)'
        exit 1
    fi

# Build the helper image without changing the normal :latest tag.
builder:
    @mkdir -p target/integration-logs
    @podman build --security-opt=label=disable --cap-add=all --device=/dev/fuse -t localhost/container-rootfs-builder:integration . >target/integration-logs/builder-manual.log 2>&1 && echo 'PASS builder (target/integration-logs/builder-manual.log)' || { echo 'FAIL builder (target/integration-logs/builder-manual.log)'; exit 1; }

# Full builds are opt-in; CASE accepts all, named cases, or a comma-separated selection.
integration case='all':
    @mkdir -p target/integration-logs
    @INTEGRATION_CASES={{ quote(case) }} cargo test --test container_integration --locked -- --ignored --nocapture >target/integration-logs/integration-suite.log 2>&1 && echo 'PASS integration (target/integration-logs/integration-suite.log)' || { echo 'FAIL integration (target/integration-logs/integration-suite.log)'; exit 1; }
