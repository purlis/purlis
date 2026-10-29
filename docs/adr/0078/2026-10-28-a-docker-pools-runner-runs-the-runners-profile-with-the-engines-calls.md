# Amended 2026-10-28: a Docker pool's runner runs the runner's profile with the engine's calls

**Ruled by the person on 2026-10-28**, on #3298 (Refs #2050), amending item 1 of this ADR's
amendment of 2026-10-27, as ADR 0089's amendment of 2026-10-28 says.

1. **The runner's seccomp profile** (`purlis_core::dockerpool::SECCOMP_PROFILE`) is Docker's
   default with four rules added: `pidfd_getfd`, as before; `clone` and `unshare` making no
   namespace but a user, a process and a network one; and `chroot`. Only the browser engine may
   make namespaces on a runner: its host starts everything else unable to (ADR 0089).
2. **The check knows the earlier profile too** (`EARLIER_SECCOMP_PROFILE`, Docker's default and
   `pidfd_getfd`, kept as it was): Docker holds a runner to either, rule for rule; Podman keeps
   the path, and each profile is kept under a file name of its own (`runner-seccomp-engine.json`
   and, as before, `runner-seccomp.json`), so the path says which; rootless Podman is also held
   to what it compiled. A runner is made with the runner's profile only where its image's
   `purlis` holds what it starts from namespaces, asked of a container of the image made for
   that alone, and with the earlier one otherwise; a runner made now that does not run the
   profile chosen for it is removed with its volume, as before, and one on the runner's profile
   whose `purlis` does not hold them is not linked (ADR 0089, D-2050-26).
3. **Its `/dev/shm` is a quarter of its memory** (`--shm-size`). The check does not hold the
   size: the memory limit, which it holds, bounds it.
