# Background: Hummingbird container construction

This is background research, not an implementation, recommendation, or promise
of a Hummingbird backend in this POC. For the related DNF5 discussion, see
[DNF5 installroot background](background-dnf5-chroot.md).

At commit
[`1649df9201bc3f03fb9a93eeb8aec569a8b55d1c`](https://gitlab.com/redhat/hummingbird/containers/-/commit/1649df9201bc3f03fb9a93eeb8aec569a8b55d1c),
Hummingbird-generated Containerfiles separate a **builder stage** from the
published output. The builder creates `${NEWROOT}`, downloads the RPMs named by
`rpms.lock.yaml`, installs them, and performs cleanup. The builder then runs
`chunkah build --rootfs ${NEWROOT}`; the final stage consumes its archive with
`FROM oci-archive:out.ociarchive`, rather than literally using `FROM scratch`.
[The generated Chunkah Containerfile](https://gitlab.com/redhat/hummingbird/containers/-/blob/1649df9201bc3f03fb9a93eeb8aec569a8b55d1c/images/chunkah/hummingbird/default/Containerfile)
and [the final-stage template](https://gitlab.com/redhat/hummingbird/containers/-/blob/1649df9201bc3f03fb9a93eeb8aec569a8b55d1c/macros/final_stage.yml.j2)
show both stages. By contrast, the template's non-Hummingbird path does emit
literal `FROM scratch`; [the generated Rawhide BIND Containerfile](https://gitlab.com/redhat/hummingbird/containers/-/blob/1649df9201bc3f03fb9a93eeb8aec569a8b55d1c/images/bind/rawhide/default/Containerfile)
is a concrete example.

## Installroot environment and RPM inputs

The builder's `dnf-installroot` wrapper first enters new mount and PID
namespaces with `unshare -f -mp`. It creates `/run`, `/proc`, `/sys`, `/dev`,
`/tmp`, and `/var`; mounts tmpfs on `/run`, `/tmp`, and `/dev`; bind-mounts
`/proc` and selected host device nodes; then invokes `dnf --installroot`.
[Its pinned script](https://gitlab.com/redhat/hummingbird/containers/-/blob/1649df9201bc3f03fb9a93eeb8aec569a8b55d1c/images/hummingbird-builder/dnf-installroot.sh)
also makes explicit that `/sys` remains empty and removes the temporary mounts
by ending the namespace.

The RPM lockfile is used to download exact listed RPM URLs, group them by
repository ID, generate local metadata with `createrepo_c`, and create
`file://` repositories with `gpgcheck=1`.
[`download-locked-packages.sh`](https://gitlab.com/redhat/hummingbird/containers/-/blob/1649df9201bc3f03fb9a93eeb8aec569a8b55d1c/images/hummingbird-builder/download-locked-packages.sh)
is the primary source for that flow. Depending on the image variant, the setup
template imports Fedora and/or Hummingbird RPM keys into the target root; see its
[lockfile, repository, and `rpmkeys --import` steps](https://gitlab.com/redhat/hummingbird/containers/-/blob/1649df9201bc3f03fb9a93eeb8aec569a8b55d1c/macros/setup_newroot.yml.j2).

## Provenance and limits

Hummingbird issue [#2](https://gitlab.com/redhat/hummingbird/containers/-/work_items/2)
identifies plain `dnf --installroot` without `/dev` and `/proc` as problematic
for package scriptlets, points to DNF5 PR
[#2270](https://github.com/rpm-software-management/dnf5/pull/2270), and names
the Fedora bootc experimental helper as the short-term working example. Its
follow-up MRs [!717](https://gitlab.com/redhat/hummingbird/containers/-/merge_requests/717)
and [!781](https://gitlab.com/redhat/hummingbird/containers/-/merge_requests/781)
respectively added that helper to the builder and used it for container builds.

The helper itself says it is based on Fedora bootc's
[`dnf-installroot`](https://gitlab.com/fedora/bootc/base-images-experimental/-/blob/6b32f89e649e7ec718a35c783f52797d9a741019/build/dnf-installroot)
script. This POC does not implement that helper, a Hummingbird output path, or
bootc finalization; the observations above must not be treated as a claim that
an output is bootable or production-supported.
