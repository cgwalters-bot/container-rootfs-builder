# Ideas

- [ ] Spike on image-embedded manifests for **pure extensions**: `FROM <base>`,
  `COPY` a JSON (or supported Kickstart) manifest into a discoverable location
  such as `/usr/lib/container-image-builder/manifests/`, then
  `RUN container-image-builder apply <manifest>`. Apply changes through existing
  package/build tooling, retain the manifest in the derived image, and test
  predictable reapplication. This mode deliberately keeps the base image's OCI
  layers; rebuilding a rootfs with `FROM scratch` is a separate use case.
