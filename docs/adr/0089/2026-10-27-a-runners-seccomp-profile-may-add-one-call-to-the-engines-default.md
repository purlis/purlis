# Amended 2026-10-27: a runner's seccomp profile may add one call to the engine's default

**Ruled by the person on 2026-10-27**, on #3410 (Refs #2019). It amends V55 as §10 reads it
(*"A seccomp profile is the engine's default or stricter"*), for a Docker pool's runner only.

1. **A Docker pool's runner runs the engine's default profile with one call allowed besides,
   `pidfd_getfd`**, which the Linux wrap's supervisor needs to make a chat's `listen` (ADR 0067,
   amended 2026-10-26). Nothing else is loosened, and every capability is still dropped. What
   the pool's check holds each engine to is ADR 0078 as amended 2026-10-27.
2. **A runner made before this profile stays usable on the engine's default**, which is
   stricter: no server a chat starts there listens until the runner is made again.
3. **A Kubernetes runner keeps `RuntimeDefault`**, which refuses the call: no server a chat
   starts there listens until #3413.
