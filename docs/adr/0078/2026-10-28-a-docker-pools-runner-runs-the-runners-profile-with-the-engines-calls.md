# Amended 2026-10-28: a Docker pool's runner runs the runner's profile with the engine's calls

**Ruled by the person on 2026-10-28**, on #3298 (Refs #2050), amending item 1 of this ADR's
amendment of 2026-10-27, as ADR 0089's amendment of 2026-10-28 says.

1. **The runner's seccomp profile** (`purlis_core::dockerpool::SECCOMP_PROFILE`) is Docker's
   default with four rules added: `pidfd_getfd`, as before; `clone` and `unshare` making no
   namespace but a user, a process and a network one; and `chroot`. A sandboxed chat is refused
   the namespace calls by its own filter.
2. **The check knows the earlier profile too** (`EARLIER_SECCOMP_PROFILE`, Docker's default and
   `pidfd_getfd`, kept as it was): Docker holds a runner to either, rule for rule; rootless
   Podman tells them apart by what it compiled; rootful Podman keeps the path alone, and its
   runner is taken as running the runner's own, whose engine then tells, by refusing to start,
   if it is not. A runner made now that does not run the runner's profile is removed with its
   volume, as before.
3. **Its `/dev/shm` is a quarter of its memory** (`--shm-size`). The check does not hold the
   size: the memory limit, which it holds, bounds it.
