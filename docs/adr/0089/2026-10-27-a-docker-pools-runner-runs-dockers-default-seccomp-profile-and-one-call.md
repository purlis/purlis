# Amended 2026-10-27: a Docker pool's runner runs Docker's default seccomp profile and one call

**Ruled by the person on 2026-10-27**, on #3410 (Refs #2019). It amends V55 as §10 reads it
(*"A seccomp profile is the engine's default or stricter"*), for a Docker pool's runner only.

1. **A Docker pool's runner runs Docker's default seccomp profile (the Moby project's
   `seccomp/v0.2.4`) with one bare `pidfd_getfd` allow besides, on every engine, Podman
   included**: Podman is given the same file, not its own default. The Linux wrap's supervisor
   needs the call to make a chat's `listen` (ADR 0067, amended 2026-10-26). Nothing else is
   loosened against Docker's default, and every capability is still dropped. What the pool's
   check holds each engine to is ADR 0078 as amended 2026-10-27.
2. **A runner made before this profile stays usable on its engine's own default**, which refuses
   the call: no server a chat starts there listens until the runner is made again. A runner made
   now must run the runner's profile.
3. **A Kubernetes runner keeps `RuntimeDefault`**, which refuses the call: no server a chat
   starts there listens until #3413.
