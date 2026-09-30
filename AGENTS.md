# Guidance for contributors

This is an experimental, source-backed Fedora rootfs planner/builder. Keep
changes conservative and document uncertainty rather than implying published
provenance or bootability without tests. See upstream [bootc](https://github.com/bootc-dev/bootc)
and [base-images](https://gitlab.com/fedora/bootc/base-images) for context.

## Checks

Run `just check-all` before submitting. This formats nothing and runs the
locked build checks, unit tests, and Clippy; detailed output is written below
`target/integration-logs/`. Run `just integration all` when executable or
container-build behavior changes.

## Rust

- Keep parsing separate from I/O so parsers can be covered by focused unit
  tests.
- Propagate errors rather than silently discarding them.
- Prefer safe `rustix`, `cap_std`, or `cap_fs_ext` APIs to `libc`; unsafe code
  remains forbidden.
- Add narrowly scoped lint exceptions only with a concrete justification.

## Source and build safety

- Keep source mappings pinned and explicit; do not silently broaden releases,
  references, or source entrypoints.
- Preserve `cap_std` capability-scoped source access and path validation.
- `.repo` files and `Containerfile.repos` builds use a disposable external
  context. Never put credentials, secrets, or private repository configuration
  in this repository or its build inputs.
- Planning and ordinary tests should remain rootless where possible. Rootfs
  composition may need privileged Podman capabilities; do not claim bootability
  without running appropriate tests.
- Do not use `/tmp` for repository work or persistent build inputs.
- Preserve this project's Apache-2.0 declaration and the official helper's
  MIT `COPYING` notice. Do not add unsupported copyright holders.

This project is entirely AI-generated and has not had human review. Require
human review before publishing or treating a build as production-supported;
an AI review does not replace it. Independently review substantial generated
changes. Use generic `Assisted-by: AI` or `Generated-by: AI` attribution and
do not advertise specific models or tools. Do not add a `Signed-off-by`
trailer on anyone's behalf. Do not commit changes unless the user explicitly
asks for a commit.
