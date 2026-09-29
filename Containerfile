# A source-backed builder; the generated rootfs is deliberately not an image layer.
ARG RUNTIME_IMAGE=quay.io/fedora/fedora@sha256:370f643e96ec63bd503f506f2b406cbc447eed2179206d0ca13ac93bde2b5e43
FROM docker.io/alpine/git:2.47.2 AS sources

ARG F43_KIWI_COMMIT=1247fcf8967eb2273a314d4703c2f907de0e6031
ARG F43_PUNGI_COMMIT=7ec704213b8d75fe173155cf04bd821f65aad66c
ARG F44_KIWI_COMMIT=dfc49a5a10f69941179fdadd96aa6a5984f7c677
ARG F44_PUNGI_COMMIT=768b06a946c4b33e3f759446d357f48773f9862f
ARG F45_KIWI_COMMIT=daf359394913068f4fc18a8b7aafc404caeb8257
ARG F45_PUNGI_COMMIT=233933230bb6a97017e937efea20fb9c4d40f948
ARG BOOTC_COMMIT=bbea58e7db3b403785d632d7db3f75bfe6cd5415
ARG ATOMIC_COMMIT=9dbdbe2f1c8009b2257201ffd4719100ff0b0ba1

RUN set -eux; \
    checkout() { \
      url="$1"; expected="$2"; destination="$3"; entrypoint="$4"; \
      work="/work/$(basename "$destination")"; mkdir -p "$work" "$destination"; shift 3; \
      git -C "$work" init; git -C "$work" remote add origin "$url"; \
      git -C "$work" fetch --depth=1 origin "$expected"; git -C "$work" checkout --detach "$expected"; \
      actual="$(git -C "$work" rev-parse HEAD)"; test "$actual" = "$expected"; \
      git -C "$work" archive HEAD | tar -x -C "$destination"; \
      manifest="$destination/.source-provenance.json"; \
      printf '{"source_url":"%s","expected_commit":"%s","source_commit":"%s","files":{' "$url" "$expected" "$actual" > "$manifest"; \
      first=1; root="$(realpath "$work")"; git -C "$work" ls-files '*.xml' "$@" > "$work/manifest-files"; \
      while IFS= read -r file; do \
        mode="$(git -C "$work" ls-files -s -- "$file" | cut -d' ' -f1)"; \
        if [ "$mode" = 120000 ]; then target="$(git -C "$work" cat-file blob "HEAD:$file")"; case "$target" in /*|../*|*/../*|'') exit 1;; esac; target_path="$(dirname "$file")/$target"; resolved="$(realpath "$work/$target_path")" || exit 1; case "$resolved" in "$root"|"$root"/*) ;; *) exit 1;; esac; test -f "$resolved"; value="symlink:$target|$(sha256sum "$resolved" | cut -d' ' -f1)"; \
         elif [ "$mode" = 100644 ] || [ "$mode" = 100755 ]; then resolved="$(realpath "$work/$file")" || exit 1; case "$resolved" in "$root"|"$root"/*) ;; *) exit 1;; esac; test -f "$resolved"; value="$(sha256sum "$resolved" | cut -d' ' -f1)"; \
        else continue; fi; \
        [ "$first" -eq 1 ] || printf ',' >> "$manifest"; printf '"%s":"%s"' "$file" "$value" >> "$manifest"; first=0; \
      done < "$work/manifest-files"; printf '}}\n' >> "$manifest"; rm -rf "$work"; test ! -e "$destination/.git"; \
    }; \
    checkout https://forge.fedoraproject.org/releng/kiwi-descriptions "$F43_KIWI_COMMIT" /sources/f43/kiwi Fedora.kiwi; \
    checkout https://forge.fedoraproject.org/releng/pungi-fedora "$F43_PUNGI_COMMIT" /sources/f43/pungi fedora-container.conf; \
    checkout https://forge.fedoraproject.org/releng/kiwi-descriptions "$F44_KIWI_COMMIT" /sources/f44/kiwi Fedora.kiwi; \
    checkout https://forge.fedoraproject.org/releng/pungi-fedora "$F44_PUNGI_COMMIT" /sources/f44/pungi fedora-container.conf; \
    checkout https://forge.fedoraproject.org/releng/kiwi-descriptions "$F45_KIWI_COMMIT" /sources/f45/kiwi Fedora.kiwi; \
    checkout https://forge.fedoraproject.org/releng/pungi-fedora "$F45_PUNGI_COMMIT" /sources/f45/pungi fedora-container.conf; \
     checkout https://gitlab.com/fedora/bootc/base-images "$BOOTC_COMMIT" /sources/bootc bootc-base-imagectl install-manifests; \
    checkout https://forge.fedoraproject.org/atomic-desktops/config "$ATOMIC_COMMIT" /sources/f45/atomic silverblue.yaml

FROM docker.io/library/rust:1.90-bookworm AS build
WORKDIR /src
COPY Cargo.toml Cargo.lock ./
COPY src ./src
RUN cargo build --release --locked

# This immutable digest selects the builder users actually run.
FROM ${RUNTIME_IMAGE}
RUN dnf install -y rpm-ostree python3 bootc selinux-policy-targeted && dnf clean all
COPY --from=build /src/target/release/container-rootfs-builder /usr/local/bin/
COPY --from=sources /sources /sources
COPY --from=sources /sources/bootc/COPYING /usr/share/licenses/bootc-base-imagectl/COPYING
RUN cd /sources/bootc && ./install-manifests && install -m 0755 bootc-base-imagectl /usr/local/bin/bootc-base-imagectl
ENTRYPOINT ["/usr/local/bin/container-rootfs-builder"]
CMD ["source", "inspect", "--from=quay.io/fedora/fedora:45", "--source-dir=/sources/f45/kiwi", "--source-dir=/sources/f45/pungi"]
