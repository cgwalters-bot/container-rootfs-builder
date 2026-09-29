# container-rootfs-builder

> **100% LLM-generated; not yet reviewed by a human; experimental.**

This is not a new Fedora build process. The goal is to wrap existing content
definitions and build processes in an interface that can run from a container
image. The CLI selects pinned upstream definitions and delegates the actual
rootfs build to existing tools, rather than replacing their semantics with a
project-owned recipe format. It does not claim published-image provenance or
bootability without verification.

Background only: [Hummingbird container construction](docs/background-hummingbird.md)
and [DNF5 installroot/chroot scope](docs/background-dnf5-chroot.md).

Today only the Fedora bootc rootfs path is executable: the pinned
[`bootc-base-imagectl`](https://gitlab.com/fedora/bootc/base-images/-/blob/bbea58e7db3b403785d632d7db3f75bfe6cd5415/bootc-base-imagectl)
delegates to `rpm-ostree compose rootfs`. KIWI/Pungi and Silverblue definitions
are plan-only. Longer term, we want to be able to use DNF or equivalent tooling
as a backend without changing the upstream definitions. The closed, unmerged
[DNF5 PR #2270](https://github.com/rpm-software-management/dnf5/pull/2270)
explored optional mount preparation for `dnf5 --installroot` inside a container
build. [The discussion](https://github.com/rpm-software-management/dnf5/pull/2270#issuecomment-2932126936)
connects that directly to bootstrapping a rootfs for the Fedora
[from-scratch flow](https://docs.fedoraproject.org/en-US/bootc/building-from-scratch/).
It is prior art for a possible DNF backend, not a merged feature or a complete
bootc rootfs finalization process.

## Plan and inspect

`--from` accepts exact, fully qualified references. KIWI and Silverblue entries
are plan-only; aliases, unqualified names, and unknown tags are rejected.

```console
container-rootfs-builder --from=quay.io/fedora/fedora:45 --plan /target-rootfs
container-rootfs-builder --from=quay.io/fedora/fedora-bootc:43 --plan /target-rootfs
container-rootfs-builder --from=quay.io/fedora/fedora-bootc:44 --plan /target-rootfs
container-rootfs-builder --from=quay.io/fedora/fedora-bootc:45 --plan /target-rootfs
podman build -t localhost/container-rootfs-builder:latest -f Containerfile .
podman run --rm localhost/container-rootfs-builder:latest
```

The builder embeds pinned F43, F44, and F45 KIWI/Pungi sources and the pinned
official Fedora [bootc/base-images](https://gitlab.com/fedora/bootc/base-images)
checkout. F43/F44 are demonstration release branches; F45 is a Beta target.
Recorded commits and internal hashes describe these checkouts, not published
images. Source contents and version status can change upstream.

## Build a Fedora rootfs

The builder installs the pinned official Python `bootc-base-imagectl` and its
manifests from commit `bbea58e7db3b403785d632d7db3f75bfe6cd5415`, plus
`rpm-ostree`, bootc, Python, and SELinux policy dependencies. It permits only
`quay.io/fedora/fedora-bootc:<43|44|45>` and runs:

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

## Development

```console
cargo fmt --check
cargo test
cargo clippy --all-targets -- -D warnings
```
