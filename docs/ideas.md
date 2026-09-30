# Ideas

## Backend boundaries

The shared layer should stay limited to exact source selection, source-root and
target validation, and Fedora release matching. Backends retain their native
semantics: Fedora bootc uses the pinned `bootc-base-imagectl`, while the
experimental F44/F45 Silverblue and Kinoite rootfs paths call
`rpm-ostree compose tree --print-only` and `rpm-ostree compose rootfs` on their
pinned native manifests. Neither Atomic Desktop path becomes a bootc image or
claims bootability.

- [ ] Verify exact published references and pinned source entrypoints before
  adding more Atomic Desktops; a registry tag alone does not
  prove that its content matches the expected release or source checkout.

For Fedora CoreOS, [fedora-coreos-config/build-rootfs](https://github.com/coreos/fedora-coreos-config/blob/42d1aba5f3f87da99b2a9ed69c6e9e06bd0c70ae/build-rootfs)
also calls the bootc helper. FCOS additionally selects stream-specific packages,
repositories, locks, and overlays, then adds image/platform/disk metadata,
postprocessing, and an initramfs. COSA promotion, signing, Kola, and installer
gates are separate. Shared rootfs orchestration is not an FCOS release path.

For RHEL 10, [image-builder's `imagetypes.yaml`](https://github.com/osbuild/image-builder/blob/aa779907c8b9193903ef7fce862da78bc015988d/data/distrodefs/rhel-10/imagetypes.yaml)
describes AMI disk partitions, bootloader and EC2 platform configuration, not
an arbitrary non-bootc OCI input. Its bootc-image-builder path expects a bootc
container. A future “non-bootc OCI to AMI” adapter would still need entitled
repositories, preserved filesystem metadata, disk/boot construction, EC2 guest
validation, and separate AWS import/registration; this POC implements none of
those steps.

## DNF5 manifest alignment

The [DNF5 manifest background](background-dnf5-manifest.md) separates a
package-resolution request from a resolved RPM manifest. It is a possible
package lock for extensions, not a replacement for native bootc or Atomic
Desktop definitions or their finalization steps.

- [ ] Prototype DNF5 manifest resolve/download with a disposable repository,
  explicitly verify RPM checksums, then compare installed NEVRAs in an
  installroot. Do not claim bootability from a package manifest alone.
- [ ] Revisit the name `manifest-to-rootfs` only if this becomes a narrow,
  manifest-input/rootfs-output tool; the current wrapper also selects native
  upstream definitions and leaves non-package build steps to their backends.

- [ ] Spike on image-embedded manifests for **pure extensions**: `FROM <base>`,
  `COPY` a JSON (or supported Kickstart) manifest into a discoverable location
  such as `/usr/lib/container-image-builder/manifests/`, then
  `RUN container-image-builder apply <manifest>`. Apply changes through existing
  package/build tooling, retain the manifest in the derived image, and test
  predictable reapplication. This mode deliberately keeps the base image's OCI
  layers; rebuilding a rootfs with `FROM scratch` is a separate use case.
