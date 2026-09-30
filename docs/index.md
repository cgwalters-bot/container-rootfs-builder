# container-rootfs-builder

This is an experimental demonstration of how the ideas in Fedora's
[`bootc-base-imagectl`](https://gitlab.com/fedora/bootc/base-images) can be
generalized into a planner and rootfs builder for **bootc** and Atomic Desktops.
It keeps the native rpm-ostree composition paths and targets a plain root filesystem: it
does not force every output through OCI image layers. OCI packaging can remain
a separate step.

The published builder image can be used to inspect a definition before doing a
rootfs build:

```console
podman run --rm ghcr.io/cgwalters-bot/container-rootfs-builder:latest \
  --from=quay.io/fedora/fedora-silverblue:44 --plan /target-rootfs
```

See [building](building.md) for the rootfs contract, repository injection, and
source pins. The [DNF5 chroot](background-dnf5-chroot.md), [DNF5 manifest](background-dnf5-manifest.md),
and [Hummingbird](background-hummingbird.md) pages describe related background.
Possible follow-on work is collected in [ideas](ideas.md).

> **Review notice:** This project is 100% LLM-generated and has not yet had
> human review. Treat it as an experiment, not production-supported software;
> automated build checks do not replace human review or boot testing.
