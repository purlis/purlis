# Amended 2026-10-26: on Linux a socket listens only where it was bound

**Built for #2019's review**, on §2 (purlis's own wrap on Linux, 2026-10-16). *Decided in
implementation; `rustix` ruled by the person on 2026-10-26; the runner's seccomp profile (item 5)
ruled by the person on 2026-10-27, amending ADR 0089 V55 (ADR 0078 and ADR 0089, amended
2026-10-27).*

1. **`listen` is the supervisor's call too.** The chat's seccomp filter hands it over, as it
   hands over `connect`. The supervisor takes the chat's own socket from the calling thread's
   own descriptor table (`pidfd_open` with `PIDFD_THREAD`, which Linux has had since 6.9, below
   the sandbox's 6.12, then `pidfd_getfd`, through `rustix`, pinned as the sandbox's other
   dependencies are), so a thread with a table of its own, or one whose process's first thread
   has ended, is judged by its own socket. It checks a second way that the socket is the one the
   thread's descriptor names (device and inode in `/proc`), and listens on that same socket
   itself. **The listen runs as the supervisor**, under its own security context: an AppArmor or
   SELinux rule written for the chat's program no longer sees it, as it already does not see the
   chat's `connect`.
2. **An internet socket the chat never bound is refused.** Landlock judges a bind the chat
   makes, not the one the kernel makes for a socket that listens unbound, so the supervisor
   refuses an internet socket bound to no port, and one whose address cannot be read. Where an
   internet socket may be bound stays the bind's to judge: the chat's Landlock layer binds no
   TCP port today. Any other socket (a unix socket, which the kernel binds to an abstract name
   where the chat did not) listens as before.
3. **The port is read again after the listen.** One other than the port judged undoes the
   listen (the socket is shut for reading, which leaves a listening socket unconnected and lets
   its port go) and refuses it. Two refusals elsewhere keep a socket judged bound from being
   unbound in between: a `connect` of any family but unix and the internet's (`AF_UNSPEC`
   disconnects) is refused, and a connected socket is handed back only once its connect is made.
4. **Fail closed, and told.** Where the chat's socket cannot be taken, nothing listens. Every
   refused listen is told as macOS's kernel tells a refused bind (`network-bind`), so the window
   raises the same block, and the refusal names why: a socket never bound, or a socket that
   could not be taken. A runtime or system that refuses `pidfd_getfd` stops every program in the
   chat that listens, unix sockets included (a `tmux` server, an `ssh` ControlMaster, a language
   server); `docs/runner-image.md` names where that happens.
5. **A Docker pool's runner is given the call** (`dockerpool::SECCOMP_PROFILE`): Docker's default
   seccomp profile allows `pidfd_getfd` only with `CAP_SYS_PTRACE`, which a runner never has. Its
   container is made with Docker's default profile and that one call besides. What the pool's
   check holds a runner to, a runner made before this profile, and Kubernetes are ADR 0078 as
   amended 2026-10-27.
