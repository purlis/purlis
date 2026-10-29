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
2. **Only the browser engine may make namespaces on a runner.** A runner's host starts
   everything it runs in a terminal (every chat, sandboxed or not, every shell) under a filter
   that refuses `clone` and `unshare` with any `CLONE_NEW…` flag (`EPERM`) and `clone3`, whose
   flags a filter cannot read (`ENOSYS`, as Docker's default answers it); what those start
   inherits it. The engine, which the host starts itself, is not under it. So no program a
   runner's host starts in a terminal makes a namespace. A harness's own sandbox that needs
   namespaces does not start there, and its chat is refused, never run without it. A desktop's
   chats are not held so: the ruling is the runner's. *Decided by the dispatcher 2026-10-28
   (D-2050-25), replacing D-2050-16's filter in the wrap.* The filter holds what a chat calls;
   it does not hold what a chat with no sandbox can change of what the runner later runs, so
   **a runner a chat ran on without the sandbox starts no browser and is filled no login until
   it is made again**, kept so on the desktop, where no runner reaches, and said in words
   (D-2050-27, the dispatcher's decision of 2026-10-29). It fails closed: a start whose answer
   is lost, or a chat taken back whose sandbox the runner does not say, counts as one with no
   sandbox; every runner kept for the pool is kept so, the ones still draining after an update
   too, since the desktop is not told which of them a link reaches; and a record that cannot be
   read is never written over. **The browsers already open on that runner end, and their data
   folders with them** (D-2050-28, the dispatcher's decision of 2026-10-29).
3. **A runner made before this profile stays usable**, on the profile it was made with
   (`Seccomp::RunnersEarlier`): its chats' servers listen, and its browsers are refused for the
   engine's sandbox until the person removes its pool and adds it again; the Notice says so.
   *Decided in implementation (D-2050-17).* **The profile goes only to a runner whose `purlis`
   holds what it starts from namespaces** (D-2050-26, the dispatcher's decision of 2026-10-29):
   the image's `purlis` is asked before its container is made, and one that does not answer as
   that `purlis` does is made with the earlier profile, its browsers refused; a runner on this
   profile whose `purlis` does not hold them is not linked. The runner runs that same `purlis`
   whatever the image's entrypoint (`--entrypoint /usr/local/bin/purlis`, under the runtime's
   `--init`). Each profile is kept under a file
   name of its own, so a Podman runner, which keeps only the path, is known by it too.
4. **A runner's `/dev/shm` is a quarter of its memory**, memory-backed and counted against it,
   as a browser pod's shared-memory volume is; the engine keeps using it, as it does on every
   other machine. *Decided in implementation (D-2050-18).*
5. **A Kubernetes runner keeps `RuntimeDefault`**, which refuses these calls without the
   capabilities a pod drops; its `purlis serve` starts what it runs in a terminal under the same
   filter, which refuses nothing more there. Its browser pods are #2059.
6. **`unshare` is refused `CLONE_NEWTIME` too**, which it alone takes: the ruling names user,
   process and network namespaces only.
