# container-rootfs-builder

> **100% LLM-generated; not yet reviewed by a human; experimental.**

Temporarily hosted under `cgwalters-bot` on GitHub; the plan is to move this
experiment to Fedora Forge. This is not an official Fedora project or release.

This is not a new Fedora build process. The goal is to wrap existing content
definitions and build processes in an interface that can run from a container
image. The CLI selects pinned upstream definitions and delegates the actual
rootfs build to existing tools, rather than replacing their semantics with a
project-owned recipe format. It does not claim published-image provenance or
bootability without verification.

Background only: [Hummingbird container construction](docs/background-hummingbird.md)
and [DNF5 installroot/chroot scope](docs/background-dnf5-chroot.md), plus the
experimental [DNF5 RPM manifest workflow](docs/background-dnf5-manifest.md).

This first spike has one shared, typed source catalog for the three Fedora bootc
release references and four F44/F45 Atomic Desktop references. The bootc path
uses the pinned
[`bootc-base-imagectl`](https://gitlab.com/fedora/bootc/base-images/-/blob/bbea58e7db3b403785d632d7db3f75bfe6cd5415/bootc-base-imagectl)
to delegate to `rpm-ostree compose rootfs`. The experimental Silverblue and
Kinoite paths invoke that rpm-ostree operation directly with their native
manifests. KIWI/Pungi definitions remain plan-only. DNF is a possible future
backend, not part of this spike; longer term it could be used
as a backend without changing the upstream definitions. The closed, unmerged
[DNF5 PR #2270](https://github.com/rpm-software-management/dnf5/pull/2270)
explored optional mount preparation for `dnf5 --installroot` inside a container
build. [The discussion](https://github.com/rpm-software-management/dnf5/pull/2270#issuecomment-2932126936)
connects that directly to bootstrapping a rootfs for the Fedora
[from-scratch flow](https://docs.fedoraproject.org/en-US/bootc/building-from-scratch/).
It is prior art for a possible DNF backend, not a merged feature or a complete
bootc rootfs finalization process.

## Plan and inspect

`--from` accepts exact, fully qualified references. KIWI/Pungi entries are
plan-only; Silverblue and Kinoite have experimental rootfs-only paths. Aliases,
unqualified names, and unknown tags are rejected.

```console
container-rootfs-builder --from=quay.io/fedora/fedora:45 --plan /target-rootfs
container-rootfs-builder --from=quay.io/fedora/fedora-bootc:43 --plan /target-rootfs
container-rootfs-builder --from=quay.io/fedora/fedora-bootc:44 --plan /target-rootfs
container-rootfs-builder --from=quay.io/fedora/fedora-bootc:45 --plan /target-rootfs
container-rootfs-builder --from=quay.io/fedora/fedora-silverblue:44 --plan /target-rootfs
container-rootfs-builder --from=quay.io/fedora/fedora-kinoite:44 --plan /target-rootfs
container-rootfs-builder --from=quay.io/fedora/fedora-silverblue:45 --plan /target-rootfs
container-rootfs-builder --from=quay.io/fedora/fedora-kinoite:45 --plan /target-rootfs
podman build -t localhost/container-rootfs-builder:latest -f Containerfile .
podman run --rm localhost/container-rootfs-builder:latest
```

Compare image filesystems with networking disabled in the helper container.
Both input images are mounted read-only, and comparison ignores timestamps,
ownership, and xattrs (these are
not implemented); it reports paths, types, symlink targets, permissions, and
streamed regular-file contents. Other Unix special files are compared by
their type and permissions, not by contents:

```console
image-diff FIRST SECOND
image-diff --helper-image localhost/container-rootfs-builder:latest FIRST SECOND
```

The builder embeds pinned F43, F44, and F45 KIWI/Pungi sources and the pinned
official Fedora [bootc/base-images](https://gitlab.com/fedora/bootc/base-images)
checkout. F43/F44 are demonstration release branches; F45 is a Beta target.
Recorded commits and internal hashes describe these checkouts, not published
images. Source contents and version status can change upstream.

## Build a Fedora rootfs

The builder installs the pinned official Python `bootc-base-imagectl` and its
manifests from commit `bbea58e7db3b403785d632d7db3f75bfe6cd5415`, plus
`rpm-ostree`, bootc, Python, and SELinux policy dependencies. For
`quay.io/fedora/fedora-bootc:<43|44|45>` it runs:

```text
bootc-base-imagectl build-rootfs --manifest=standard SOURCE_ROOT TARGET
```

`SOURCE_ROOT` is an absolute mounted Fedora repository-image root. By default
its `/etc/os-release` release must match `--from`; `--releasever` selection is
unsupported. `--source-root-rw` is required because the helper may update RPM
metadata; use it only with a disposable source image.

`Containerfile.rootfs` defaults to Fedora 44 for both the bootc image and the
official repository image. The digest is a `linux/amd64` selection:

```console
podman build --security-opt=label=disable --cap-add=all --device=/dev/fuse \
  --build-arg CONFIG_BUILDER=localhost/container-rootfs-builder:latest \
  --build-arg REPOS_IMAGE=quay.io/fedora/fedora@sha256:8938dce2600de0b78f5ef8d1541192f207fdafb7414d83957f6687147aa8998b \
  -f Containerfile.rootfs -t localhost/fedora-bootc-from-scratch:44 .
```

To inject selected repositories, use a separate disposable context; never add
credentials or `.repo` files here. `Containerfile.repos` is the sample image
definition. Create `repo-image-context/repos/fedora44-sample.repo` with real
Fedora metalink settings, then run:

```console
mkdir -p repo-image-context/repos
cp Containerfile.repos repo-image-context/Containerfile
cat > repo-image-context/repos/fedora44-sample.repo <<'EOF'
[fedora44-sample]
name=Fedora 44 sample repository ($basearch)
metalink=https://mirrors.fedoraproject.org/metalink?repo=fedora-44&arch=$basearch
enabled=1
gpgcheck=1
gpgkey=file:///etc/pki/rpm-gpg/RPM-GPG-KEY-fedora-44-$basearch
EOF
podman build -f repo-image-context/Containerfile -t localhost/fedora-44-selected-repos:local repo-image-context
podman run --rm localhost/fedora-44-selected-repos:local dnf repolist --enabled fedora44-sample
podman build --security-opt=label=disable --cap-add=all --device=/dev/fuse \
  --build-arg CONFIG_BUILDER=localhost/container-rootfs-builder:latest \
  --build-arg REPOS_IMAGE=localhost/fedora-44-selected-repos:local \
  -f Containerfile.rootfs -t localhost/fedora-bootc-from-scratch:44 .
```

The source image must still identify Fedora 44. Matching `--from` and source
image is the supported path; `--allow-cross-release` is intentionally not
wired into this workflow. Verify outputs rather than assuming parity:

```console
podman run --rm --entrypoint rpm localhost/fedora-bootc-from-scratch:44 -E '%{fedora}'
podman run --rm --security-opt=label=disable --cap-add=all --device=/dev/fuse --entrypoint bootc localhost/fedora-bootc-from-scratch:44 container lint
```

Rootless Podman has been exercised; other hosts may require the shown
capabilities or device access. See the upstream [bootc documentation](https://bootc.dev/),
[bootc-dev/bootc](https://github.com/bootc-dev/bootc), and
[base-images](https://gitlab.com/fedora/bootc/base-images).

### Experimental Atomic Desktop rootfs

Silverblue and Kinoite references for Fedora 44 and 45 are enabled, using the
complete pinned Atomic Desktops checkouts at commits
`1a1effa1ae6ef22c961ff5962ec314d9208231e1` (f44) and
`9dbdbe2f1c8009b2257201ffd4719100ff0b0ba1` (f45). After the shared target and
Fedora release checks, `rpm-ostree compose tree --print-only` checks the treefile,
its includes, and referenced files; repository resolution happens only during
the subsequent
`rpm-ostree compose rootfs --source-root-rw=/repos` with that manifest and the
requested target. This deliberately leaves manifest includes and non-YAML
inputs to rpm-ostree and the pinned checkout; it does not parse or translate
the Atomic Desktop configuration.

To exercise either Fedora 44 path using a disposable matching repository source
image (the digest selects `linux/amd64`), leave `FEDORA_RELEASE` at 44 and
`ATOMIC_VARIANT` at `silverblue`, or set the variant to `kinoite`:

```console
podman build --security-opt=label=disable --cap-add=all --device=/dev/fuse \
  --build-arg CONFIG_BUILDER=localhost/container-rootfs-builder:latest \
  --build-arg REPOS_IMAGE=quay.io/fedora/fedora@sha256:8938dce2600de0b78f5ef8d1541192f207fdafb7414d83957f6687147aa8998b \
  --build-arg ATOMIC_VARIANT=kinoite \
  -f Containerfile.atomic-rootfs -t localhost/kinoite-rootfs-experiment:44 .
podman run --rm --entrypoint rpm localhost/kinoite-rootfs-experiment:44 -E '%{fedora}'
```

This is an experimental plain filesystem rootfs only. The container build is
transport for that rootfs, not the default product or an image-compose API.
`FROM scratch` plus `COPY` flattens it into a filesystem layer; it does **not**
preserve the OSTree commit metadata, OCI configuration (including the
treefile's command), or package-aware layering produced by Silverblue's upstream
`rpm-ostree compose image` pipeline. `Containerfile.atomic-rootfs` does not add
bootc or ostree bootability labels or a command. Do not claim bootability or
report `bootc container lint` as a gate for this output. These source mappings
are not claims that the experiment reproduces published OCI images. Other
desktops have not been mapped or built.

## Development

```console
cargo fmt --check
cargo test
cargo clippy --all-targets -- -D warnings
```

### Opt-in container integration

`just integration kinoite44` runs the ignored container suite for one case;
`just integration` runs `bootc44`, `silverblue44`, and `kinoite44` sequentially.
The `integration` argument also accepts comma-separated cases. For direct
Cargo use, `INTEGRATION_CASES=bootc44,kinoite44 cargo test --test
container_integration -- --ignored --nocapture` selects a subset. It requires Fedora Linux, rootless-capable Podman, `just`, network
access, `/dev/fuse`, and several GB of temporary image/build space. The suite
builds a current-tree helper image, uses the pinned repository image and the
same `Containerfile.rootfs`/`Containerfile.atomic-rootfs` capabilities shown
above, and records full command output under `target/integration-logs/` while
printing only pass/fail summaries. Atomic Desktop checks are rootfs checks, not
boot tests; future bcvk/VM boot tests are intentionally separate.
