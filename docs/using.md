# Using the builder

The builder consumes pinned Fedora definitions and invokes their native tools
to produce a root filesystem. This guide covers choosing an input, running the
build, and supplying repository configuration.

## Published builder

GitHub Actions builds native amd64 and arm64 builder images using the
[bootc-dev/infra publication pattern](https://github.com/bootc-dev/infra/blob/5dd3ce6af7b945c5a776dd154d7f2ead4fde6f8/.github/workflows/build-devcontainer.yml).
Only a successful `main` push can publish: PRs have no registry-write job.
The workflow merges the two digests into `latest` and `sha-<git commit>` tags.
Use a digest or commit tag when replaying a build; `latest` moves.

GHCR makes a new package private by default, even for a public repository.
After the first push, the package owner must set its visibility to public in
GitHub's package settings before anonymous pulls work. The source annotation
links the package to this repo but does not change visibility.

## Source catalog

The executable catalog accepts these fully qualified references:

```text
quay.io/fedora/fedora-bootc:43
quay.io/fedora/fedora-bootc:44
quay.io/fedora/fedora-bootc:45
quay.io/fedora/fedora-silverblue:44
quay.io/fedora/fedora-kinoite:44
quay.io/fedora/fedora-silverblue:45
quay.io/fedora/fedora-kinoite:45
```

`quay.io/fedora/fedora:45` is also accepted for planning and source inspection,
but its KIWI/Pungi build path is not implemented. Aliases, short names, and
unknown tags fail. The bootc entries use the
`fedora-standard.yaml` definition from
[`fedora/bootc/base-images`](https://gitlab.com/fedora/bootc/base-images),
ref `main`, pinned at `bbea58e7db3b403785d632d7db3f75bfe6cd5415`. The native
backend is `bootc-base-imagectl build-rootfs --manifest=standard`, which in
turn invokes `rpm-ostree compose rootfs`.

The Atomic Desktop entries use the native `silverblue.yaml` or `kinoite.yaml`
from [`atomic-desktops/config`](https://forge.fedoraproject.org/atomic-desktops/config):
ref `f44` at
`1a1effa1ae6ef22c961ff5962ec314d9208231e1`, and ref `f45` at
`9dbdbe2f1c8009b2257201ffd4719100ff0b0ba1`. The backend validates the native
treefile with `rpm-ostree compose tree --print-only`, then runs
`rpm-ostree compose rootfs` against the requested target.

The image also contains pinned F43, F44, and F45 KIWI and Pungi sources for
inspection. They are not rootfs build inputs in this demo. The current source
library pins are KIWI `1247fcf8967eb2273a314d4703c2f907de0e6031`,
`dfc49a5a10f69941179fdadd96aa6a5984f7c677`, and
`daf359394913068f4fc18a8b7aafc404caeb8257`, with Pungi
`7ec704213b8d75fe173155cf04bd821f65aad66c`,
`768b06a946c4b33e3f759446d357f48773f9862f`, and
`233933230bb6a97017e937efea20fb9c4d40f948` for F43 through F45.

Source metadata records candidate mappings and internal file hashes. It is not
Git provenance, published-image provenance, or evidence that generated output
matches a published Fedora image. Release and architecture observations are
checks on the selected inputs, not a promise about registry retention.

## Bootc rootfs build

`Containerfile.rootfs` mounts a repository image at `/repos` and runs:

```text
container-rootfs-builder build-rootfs \
  --from=quay.io/fedora/fedora-bootc:44 \
  --source-root=/repos --source-root-rw --target=/target-rootfs
```

The source root must identify Fedora and match the requested release. The
`--source-root-rw` switch is required because the native backend may update RPM
metadata; use it only with a disposable source image. Cross-release builds are
not wired into the Containerfiles. The default repository-image pin is
`quay.io/fedora/fedora@sha256:80d49c6c7c4303efb5eebc0317e343588d5d482146ca2be48eb82494d2a83060`,
used by the tested `linux/amd64` builds. Verify platforms and release metadata when
changing pins. The builder's default F45 runtime image remains
`quay.io/fedora/fedora@sha256:e1e716b2f6ca98e1ed41e62d32a7bcbf1637d5b931894299eab9c659a16fe8ed`;
these are current pins, not permanent registry or release claims.

The generated filesystem is copied from `scratch`. `Containerfile.rootfs`
adds bootc/OSTree-related OCI metadata for its experimental bootc path, but no
VM boot test is implied. The Atomic Desktop file deliberately produces only a
plain filesystem rootfs and does not add those labels or a command. Neither
path is a published equivalent of Silverblue, Kinoite, or Fedora bootc.

## Disposable repository injection

Keep repository configuration outside this checkout. The sample
`Containerfile.repos` expects an external context containing `repos/*.repo`.
For example:

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
podman build -f repo-image-context/Containerfile \
  -t localhost/fedora-44-selected-repos:local repo-image-context
podman run --rm localhost/fedora-44-selected-repos:local \
  dnf repolist --enabled fedora44-sample
```

Pass that image as `--build-arg REPOS_IMAGE=localhost/fedora-44-selected-repos:local`
to either rootfs build. Never commit credentials, private repository files, or
`.repo` files here. The repository image must still match `--from`.

## Architecture and verification

The GHCR publication workflow builds `amd64` and `arm64` builder images.
Current rootfs integration evidence is x86_64 only;
the `linux/amd64` repository pin and the Podman `/dev/fuse` examples are
specific to that path. No arm rootfs integration or VM boot test has been run.

Use `just integration bootc44`, `just integration silverblue44`, or
`just integration kinoite44` for the opt-in cases. Full output goes to
`target/integration-logs/`. These checks exercise filesystem composition and
metadata assertions only; they do not establish bootability or published-image
parity.
