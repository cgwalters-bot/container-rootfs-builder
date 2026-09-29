# Background: DNF5 installroot and chroot scope

This page records prior art only. It does not implement, endorse, or promise a
generic chroot facility, a DNF5 backend, or a complete bootable-image builder
in this POC. See also [Hummingbird container construction](background-hummingbird.md).

DNF5 PR [#2270](https://github.com/rpm-software-management/dnf5/pull/2270) is
closed, was never merged, and was explicitly a WIP proposal for
`--with-mounts` alongside an absolute `--installroot`. Its proposed behavior
was to prepare special mounts in the target root; it is not functionality that
can be attributed to DNF5 today. The PR's API record confirms `state: closed`
and `merged: false`, while [the proposed implementation commit](https://github.com/rpm-software-management/dnf5/commit/6a112664014807fd02d079e4d90a083db5273f76)
describes the intended mount setup.

## What the proposal was, and was not

Ordinary `dnf --installroot` changes DNF's target root; it does not by itself
mean that target has a mounted `/proc`, a usable device tree, or the other
runtime context needed by arbitrary chrooted programs. The proposed
`--with-mounts` work would create target directories, mount temporary filesystems,
bind selected host devices and `/proc`, and add standard-stream symlinks.
[PR #2270's implementation description](https://github.com/rpm-software-management/dnf5/pull/2270/commits/6a112664014807fd02d079e4d90a083db5273f76)
lists those actions.

The distinction matters for mount lifetime and isolation. A DNF maintainer
recommended a [private mount namespace](https://github.com/rpm-software-management/dnf5/pull/2270#issuecomment-2923586653)
so mounts are cleaned up when DNF exits; the Fedora bootc experimental
[`dnf-installroot`](https://gitlab.com/fedora/bootc/base-images-experimental/-/blob/6b32f89e649e7ec718a35c783f52797d9a741019/build/dnf-installroot)
script uses `unshare -f -mp`, mounts `/run`, `/tmp`, and `/dev`, and bind-mounts
`/proc`. The helper does create a PID namespace; it is still not a full
container or general chroot facility. By contrast, cgwalters described the
proposed DNF mount setup as not needing a nested PID namespace, cgroups, or
seccomp in the [PR discussion](https://github.com/rpm-software-management/dnf5/pull/2270#issuecomment-2932126936).

## Why it remained prior art

cgwalters connected the proposal to bootstrapping an initial filesystem tree in
a nested container build and then using it in later native layers, citing the
[bootc from-scratch example](https://docs.fedoraproject.org/en-US/bootc/building-from-scratch/#_example_generating_a_from_scratch_base_image_with_exact_version_control).
That relation explains why PR #2270 is relevant background, not evidence of a
complete bootable-image builder or bootc finalization path.

Reviewers also raised scope and side-effect concerns. The mkosi maintainer said
mkosi still needs its own logic for other package managers and generic chroot
commands, and opposed making the behavior default without environment checks;
see [that review comment](https://github.com/rpm-software-management/dnf5/pull/2270#issuecomment-2922372894).
DNF maintainers separately warned that mount requirements vary by distribution,
may interact with SELinux, and could become an unbounded support burden; see
[the scope objection](https://github.com/rpm-software-management/dnf5/pull/2270#issuecomment-2929539887)
and [the maintainer follow-up](https://github.com/rpm-software-management/dnf5/pull/2270#issuecomment-2932010387).

Hummingbird issue [#2](https://gitlab.com/redhat/hummingbird/containers/-/work_items/2)
provides the direct provenance link: it calls out a plain installroot lacking
`/dev` and `/proc`, references PR #2270, and points to the Fedora helper as the
working short-term example. This POC implements none of that work.
