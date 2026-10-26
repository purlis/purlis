# Amended 2026-10-26: on Linux a socket listens only where it was bound

**Built for #2019's review**, on §2 (purlis's own wrap on Linux, 2026-10-16). *Decided in
implementation; `rustix` ruled by the person on 2026-10-26.*

1. **`listen` is the supervisor's call too.** The chat's seccomp filter hands it over, as it
   hands over `connect`. The supervisor takes the chat's own socket from its descriptor table
   (`pidfd_open` and `pidfd_getfd`, through `rustix`, pinned as the sandbox's other dependencies
   are), reads where it is bound, and listens on that same socket itself.
2. **A socket the chat never bound is refused.** Landlock judges a bind the chat makes, not the
   one the kernel makes for a socket that listens unbound, so the supervisor refuses an
   internet socket bound to no port. A socket the chat bound itself (a unix socket, or an
   internet one whose bind Landlock allowed) listens as before.
3. **Fail closed.** Where the chat's socket cannot be taken (a container runtime whose seccomp
   profile refuses `pidfd_getfd`), nothing listens.
