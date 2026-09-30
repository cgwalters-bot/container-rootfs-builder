# Background: DNF5 RPM manifests

This is research for a possible package-resolution backend, **not** a DNF5
backend in this POC. The [DNF5 manifest command](https://github.com/rpm-software-management/dnf5/blob/1b2de4bdc2f25a6de2884fb2cf81f80a8ce2a567/doc/dnf5_plugins/manifest.8.rst)
is explicitly experimental. It uses [libpkgmanifest](https://github.com/rpm-software-management/libpkgmanifest/tree/41d682765f3f12e7ae4d324227b9bf9e9703a587),
whose input and resolved-manifest schemas describe **RPM content**, not a
complete image definition.

The default `rpms.in.yaml` is a **prototype-format** input with
`contentOrigin.repos`, a package list, and `arches`; the plugin calls
[`parse_prototype`](https://github.com/rpm-software-management/dnf5/blob/1b2de4bdc2f25a6de2884fb2cf81f80a8ce2a567/dnf5-plugins/manifest_plugin/manifest_resolve.cpp#L75-L82)
to convert its supported subset. The separate
[native libpkgmanifest input schema](https://github.com/rpm-software-management/libpkgmanifest/blob/41d682765f3f12e7ae4d324227b9bf9e9703a587/schemas/input.json)
models repositories and package `install`/`reinstall` requests directly.
`dnf5 manifest resolve` solves the prototype request and writes
`packages.manifest.yaml`. The
[resolved schema](https://github.com/rpm-software-management/libpkgmanifest/blob/41d682765f3f12e7ae4d324227b9bf9e9703a587/schemas/manifest.json)
groups packages by architecture and records their names, EVRs, repository IDs,
locations, and optionally checksums. `manifest new` can instead capture a
resolved package set from specs or an installed system.

The distinction between **resolution and consumption** matters:
[`manifest download`](https://github.com/rpm-software-management/dnf5/blob/1b2de4bdc2f25a6de2884fb2cf81f80a8ce2a567/dnf5-plugins/manifest_plugin/manifest_download.cpp)
matches available RPMs against recorded checksums when present, whereas the
current [`manifest install`](https://github.com/rpm-software-management/dnf5/blob/1b2de4bdc2f25a6de2884fb2cf81f80a8ce2a567/dnf5-plugins/manifest_plugin/manifest_install.cpp)
selects recorded NEVRAs and repository IDs but does not explicitly compare
manifest checksums before installation. Do not advertise the latter as
checksum-enforced installation.

## Build policy is separate from the package list

The native input schema only exposes `allow_erasing` under `options` (the
prototype converter maps `allowerasing` to it); neither input encodes whether
documentation is installed. DNF5 does have a global
[`--no-docs` option](https://github.com/rpm-software-management/dnf5/blob/1b2de4bdc2f25a6de2884fb2cf81f80a8ce2a567/dnf5/main.cpp#L357-L370),
implemented as [`tsflags=nodocs`](https://github.com/rpm-software-management/dnf5/blob/1b2de4bdc2f25a6de2884fb2cf81f80a8ce2a567/doc/dnf5.conf.5.rst#L634-L654).
Likewise `install_weak_deps` affects package selection but lives in DNF
configuration, not the manifest input schema. A future builder must keep
resolution and installation policy consistent and record choices such as
docs, weak dependencies, architecture, repository configuration, and release.
For a fresh [`--installroot`](https://github.com/rpm-software-management/dnf5/blob/1b2de4bdc2f25a6de2884fb2cf81f80a8ce2a567/doc/misc/installroot.7.rst#L30-L59),
release and repository configuration also need explicit handling; the target
has no RPM database from which to infer `$releasever` yet.

## Possible fit

A later DNF backend could accept a separately resolved manifest as a package
lock for **pure extensions**, keeping the original upstream image definition
alongside it. A small experiment would resolve/download from a disposable
repository, independently check RPM hashes, then compare installed NEVRAs in
an explicit installroot. We must not silently translate Fedora bootc or Atomic
Desktop treefiles into this package-only schema: their postprocessing,
initramfs, OSTree layout, and image metadata remain native backend concerns.

Neither the plugin nor libpkgmanifest creates the `/proc` and `/dev` mounts
package scriptlets may require, finalizes a bootc rootfs, or builds an OCI
image. The separate, closed [DNF5 mount proposal](background-dnf5-chroot.md)
and [Hummingbird's pipeline](background-hummingbird.md) show why those steps
need their own explicit contract. `manifest-to-rootfs` would therefore be too
narrow as the umbrella project name unless this tool's scope changes.
