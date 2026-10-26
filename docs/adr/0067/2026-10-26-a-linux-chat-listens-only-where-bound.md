# Amended 2026-10-26: on Linux a socket listens only where it was bound

**Built for #2019's review**, on §2 (purlis's own wrap on Linux, 2026-10-16). *Decided in
implementation; `rustix`, and the runner's seccomp profile, ruled by the person on 2026-10-26.*

1. **`listen` is the supervisor's call too.** The chat's seccomp filter hands it over, as it
   hands over `connect`. The supervisor takes the chat's own socket from its process's
   descriptor table (`pidfd_open` and `pidfd_getfd`, through `rustix`, pinned as the sandbox's
   other dependencies are), checks it is the one the calling thread's descriptor names (a thread
   may hold a table of its own), and listens on that same socket itself.
2. **An internet socket the chat never bound is refused.** Landlock judges a bind the chat
   makes, not the one the kernel makes for a socket that listens unbound, so the supervisor
   refuses an internet socket bound to no port. Where an internet socket may be bound stays the
   bind's to judge: the chat's Landlock layer binds no TCP port today. Any other socket (a unix
   socket, which the kernel binds to an abstract name where the chat did not) listens as before.
3. **Fail closed.** Where the chat's socket cannot be taken, nothing listens.
4. **A Docker pool's runner is given the call** (`dockerpool::SECCOMP_PROFILE`): Docker's default
   seccomp profile allows `pidfd_getfd` only with `CAP_SYS_PTRACE`, which a runner never has. Its
   container is made with Docker's default profile and that one call besides, and the pool's
   check holds every runner to exactly that profile. A Kubernetes runner is #3413.
