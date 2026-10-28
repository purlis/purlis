# Amended 2026-10-28: a Docker pool's runner lets the browser engine make its sandbox

**Ruled by the person on 2026-10-28**, on #3298 (Refs #2050). It amends V55 as §10 reads it
(*"A seccomp profile is the engine's default or stricter"*), and this ADR's amendment of
2026-10-27, for a Docker pool's runner only. It is the Docker twin of what spec #2041 rules for
a cluster's browser pods (*"the runtime default plus what Chrome's sandbox needs"*).

1. **A Docker pool's runner also allows what the browser engine's sandbox needs to start**:
   `clone` and `unshare` that make no namespace but a user, a process and a network one
   (`CLONE_NEWNS`, `CLONE_NEWCGROUP`, `CLONE_NEWUTS` and `CLONE_NEWIPC` refused), and `chroot`,
   none behind a capability. Under Docker's default with `pidfd_getfd` alone, the pinned engine
   ends at its start for want of a sandbox; with these three it starts with its sandbox on, and
   with any one of them left out, or the network or the process namespace refused too, it does
   not (measured 2026-10-28). purlis never runs the engine without its sandbox. Every
   capability is still dropped and no new privileges are gained. `chroot` reaches nothing
   outside a user namespace a process made itself. *Decided in implementation (D-2050-15).*
2. **A chat in purlis's own Linux sandbox makes no namespace.** The sandbox's seccomp filter
   refuses `clone` and `unshare` with any `CLONE_NEW…` flag (`EPERM`), and `clone3`, whose
   flags it cannot read (`ENOSYS`, as Docker's default answers it), so such a chat beside the
   engine reaches no more of the kernel than before. The filter is the same on every machine,
   so a runner chat is sandboxed exactly as a local one (spec #2041). *Decided in
   implementation (D-2050-16).*
3. **A runner made before this profile stays usable**, on the profile it was made with
   (`Seccomp::RunnersEarlier`): its chats' servers listen, and its browsers are refused for the
   engine's sandbox until the person removes its pool and adds it again; the Notice says so. A
   runner made now must run this profile. *Decided in implementation (D-2050-17).*
4. **A runner's `/dev/shm` is a quarter of its memory**, memory-backed and counted against it,
   as a browser pod's shared-memory volume is; the engine keeps using it, as it does on every
   other machine. *Decided in implementation (D-2050-18).*
5. **A Kubernetes runner is unchanged**: its browser pods are #2059.
