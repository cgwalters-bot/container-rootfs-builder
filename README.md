# container-rootfs-builder

> **100% LLM-generated; not yet reviewed by a human; experimental.**

This demo extends Fedora's
[`bootc-base-imagectl`](https://gitlab.com/fedora/bootc/base-images) into one
container-runnable interface for Fedora bootc, Silverblue, and Kinoite content
definitions. It ships pinned upstream configurations and runs their existing
rpm-ostree build paths. It is a wrapper, not a new Fedora build system.

The common output is a root filesystem. Definitions and build tooling can be
shared without sharing physical image layers; OCI packaging remains a separate
step. The catalog can grow to cover other definitions and backends without
turning them into a lowest-common-denominator package list.

GitHub is temporary hosting ahead of a planned move to Fedora Forge. This is
not an official Fedora project, and generated rootfs outputs are not certified
equivalents of Fedora's published images or VM-boot tested.

## Try the planner

CI builds the builder for `linux/amd64` and `linux/arm64` and publishes it to
GHCR after the tests pass on `main`. Rootfs integration tests currently run on
x86_64; publishing an arm64 builder does not establish arm64 rootfs coverage.

```console
podman run --rm ghcr.io/cgwalters-bot/container-rootfs-builder:latest \
  --from=quay.io/fedora/fedora-silverblue:44 --plan /target-rootfs
```

The accepted executable references are exact: bootc 43, 44, and 45, plus
Silverblue and Kinoite 44 and 45. Aliases, unqualified names, and unknown
references fail rather than guessing a source mapping.

## Build a rootfs

From a checkout, the default `Containerfile.rootfs` path builds the Fedora 44
bootc rootfs. The Atomic Desktop counterpart defaults to Silverblue 44. Both
commands below use the published builder; rpm-ostree's nested build environment
needs the shown capabilities and device access.

```console
podman build --security-opt=label=disable --cap-add=all --device=/dev/fuse \
  --build-arg CONFIG_BUILDER=ghcr.io/cgwalters-bot/container-rootfs-builder:latest \
  -f Containerfile.rootfs -t localhost/fedora-bootc-rootfs:44 .

podman build --security-opt=label=disable --cap-add=all --device=/dev/fuse \
  --build-arg CONFIG_BUILDER=ghcr.io/cgwalters-bot/container-rootfs-builder:latest \
  -f Containerfile.atomic-rootfs -t localhost/fedora-silverblue-rootfs:44 .
```

Select Kinoite with `--build-arg ATOMIC_VARIANT=kinoite`. To change the Fedora
release, select both `FEDORA_RELEASE` and a matching `REPOS_IMAGE`. See
[building details](docs/building.md) for repository injection, source pins,
architecture selection, and the rootfs output contract.

## Development

```console
just check-all
just integration all
```

Container integration is opt-in and requires Podman, network access, `/dev/fuse`,
and several GB of storage. Detailed logs go to `target/integration-logs/`.

For a focused filesystem comparison, see
[`image-diff`](src/bin/image-diff.rs), for example
`image-diff FIRST SECOND`. It compares mounted image filesystems with networking
disabled in the helper container;
it is not a bootability check.

The [DNF5 chroot](docs/background-dnf5-chroot.md), [DNF5 manifest](docs/background-dnf5-manifest.md),
and [Hummingbird](docs/background-hummingbird.md) pages provide background; [ideas](docs/ideas.md)
records possible future backends and scope.

This project is licensed under Apache-2.0. The upstream
`bootc-base-imagectl` helper's MIT `COPYING` notice is retained in the image.

## Documentation

Read the [project site](https://cgwalters-bot.github.io/container-rootfs-builder/)
for the overview and background. Build it locally with `just docs-check`.
