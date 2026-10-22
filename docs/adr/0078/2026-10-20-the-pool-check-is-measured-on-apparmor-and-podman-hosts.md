# The pool check is measured on AppArmor and Podman hosts, 2026-10-20

This note adds to *A Docker pool's runner is a confined container with a key of its own*
(#2506) and changes none of its rulings. Item 6 is **the person's ruling of 2026-10-20**; the
others are **decided in implementation, to be confirmed** (D-2680-1 to D-2680-5).

1. **Each host the check is meant for is measured on CI** (#2680). A workflow of its own makes a
   container the pool's way on Docker on an AppArmor host (GitHub's Ubuntu 24.04), on rootful
   Podman and on rootless Podman. It records what `inspect` and `info` write, holds the check to
   it, and makes seven loosened containers on each: privileged, with a capability added,
   AppArmor unconfined, seccomp unconfined, in the machine's process namespace, in the
   machine's users, and on the machine's network. Each is refused, but for the machine's users
   on Podman: Podman's own user namespace is the machine's unless another is asked for, so
   `--userns host` makes the pool's container word for word, as measured. The recorded copies, cut to
   the fields the check reads, are `crates/purlis-core/src/dockerpool/hosts/`, which the unit
   tests read; the whole of what each host wrote is the run's artifact. *D-2680-1.*
2. **A Podman container is held to Podman's words, told by the field Docker never writes**
   (`OCIRuntime`). Its capabilities are none effective and none bounding, where Docker writes
   `CapDrop: ["ALL"]`. Its own namespaces are `private`, and its cgroup namespace is `CgroupMode`.
   It has no device requests field. Its home is the one bind `<volume>:<home>:rw,rprivate,nosuid,nodev,rbind`.
   Its network is `podman`. Its `/tmp` carries `,rprivate,tmpcopyup` after the pool's options.
   Its runtime is `oci` with `crun` or `runc` beside it. Its AppArmor profile is
   `containers-default-<version>` (digits and dots, with `-apparmor<digits>` where the
   distribution adds it), or none on rootless Podman alone, which the daemon's check says it is.
   Each is exactly what was measured, and anything else is refused. *D-2680-2.*
3. **Podman's daemon check is its own.** Podman has no `SecurityOptions`, so where Docker's
   question fails, `info --format '{{json .Host.Security}}'` is asked. A Podman makes runners
   with seccomp on and the profile it ships (`/usr/share/containers/seccomp.json`), rootful or
   rootless, and says which. *D-2680-3.*
4. **Rootless Podman is taken.** It loads no AppArmor profile for a container, and its user
   namespace is the wall instead, which ADR 0089 §10 (V55) prefers for a local runner. *Rejected:*
   refusing it, which would go against that ruling. *D-2680-4.*
5. **No SELinux host is measured.** GitHub's hosted runners are Ubuntu, macOS and Windows, and a
   Fedora container on Ubuntu runs Ubuntu's kernel. What an SELinux host writes (a process and a
   mount label) is refused until one is measured.
6. **Podman's own folders are never a chat's to write** (the person's ruling). Its settings
   (`containers/` in `$XDG_CONFIG_HOME` and `~/.config`), its storage (`containers/` in
   `$XDG_DATA_HOME` and `~/.local/share`) and its run state (`containers/` and `libpod/` in
   `$XDG_RUNTIME_DIR`) are denied to every chat's writes as later code, each with everything
   below it, under every spelling on macOS. Rootless stays the preferred engine for a local
   runner, as V55 has it.
