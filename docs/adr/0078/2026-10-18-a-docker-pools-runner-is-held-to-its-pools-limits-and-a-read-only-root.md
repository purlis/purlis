# Amended 2026-10-18: a Docker pool's runner is held to its pool's limits and a read-only root

**Built by #2506** on items 1 and 2 of the amendment of 2026-10-17 on a Docker pool's runner,
which the operator ruled that day. It adds to them and changes neither. *Decided in
implementation, to be confirmed* (D-2506-1 to D-2506-3).

1. **Every runner container is held to its pool's limits** (`purlis_core::dockerpool::Limits`,
   spec #2041: a pool's limits are configurable per pool): its CPUs (`--cpus`), its memory
   (`--memory`, with `--memory-swap` the same, so no swap beyond it), its processes
   (`--pids-limit`) and the size of its `/tmp`. The defaults are two CPUs, 4 GiB, 1024
   processes and 1 GiB of `/tmp`. A pool's limits leave a runner room to start a chat: at least
   512 MiB and 64 processes, and a `/tmp` no larger than the memory it counts against.
2. **Its root file system is read-only** (`--read-only`), so its home volume is the only place
   it can write that may hold a program it runs. `/tmp` is a `tmpfs` mounted
   `rw,nosuid,nodev,noexec` at its pool's size, and Docker's own `/dev/shm` is memory-backed and
   `noexec` too.
3. **The check holds both** (item 2): what `docker inspect` says must be a read-only root, the
   one `tmpfs` at `/tmp` with exactly those options, the CPU, memory, swap and process limits the
   runner was made with, and the out-of-memory killer left on. Anything else, a field that
   cannot be read included, refuses the link.
