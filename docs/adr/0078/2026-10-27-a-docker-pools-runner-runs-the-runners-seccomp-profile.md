# Amended 2026-10-27: a Docker pool's runner runs the runner's seccomp profile

**Ruled by the person on 2026-10-27**, on #3410 (Refs #2019), amending item 1 of the ruling of
2026-10-17 (*"Docker's default seccomp and AppArmor profiles, never named"*) and ADR 0089 V55 as
ADR 0089's amendment of 2026-10-27 says. The ruling's three conditions are items 3, 4 and 5.

1. **A runner's container is made with the runner's seccomp profile**
   (`purlis_core::dockerpool::SECCOMP_PROFILE`): Docker's default profile, the Moby project's
   `seccomp/v0.2.4` kept beside it as published, with one rule added, `pidfd_getfd` allowed
   with no condition. The Linux wrap's supervisor takes a chat's socket with it to make the
   chat's `listen` (ADR 0067, amended 2026-10-26). The call gives a process in the runner no
   reach it does not have already: the profile allows `ptrace` and `process_vm_readv`, which the
   same kernel check guards, and every capability is dropped. The AppArmor profile stays the
   runtime's default, never named.
2. **The check holds a runner to that profile, as each engine lets it be read.**
   - **Docker** keeps the profile it was given: it is held to the runner's profile rule for
     rule.
   - **Rootless Podman** keeps the path it read the profile from, and what it compiled from it in
     its runtime's config, which this user can read: the path must be absolute and name the
     runner's profile's file, and the compiled profile must be exactly what Podman makes of the
     runner's for a container with no capability on this machine's architecture.
   - **Rootful Podman** keeps only that path, and its runtime's config is root's: it is held to
     the path alone. The container's id is pinned when purlis makes it, and no engine changes a
     container's seccomp profile in place, so the path is the one purlis gave it.
   - The runtime's own default (item 3) is taken too; `seccomp=unconfined` and any other profile
     are refused.
3. **A runner made before the profile stays usable on the runtime's own default.** That profile
   is stricter: it refuses `pidfd_getfd`, so no server a chat starts on that runner listens, and
   the refusal is told. The runner listens once the person removes it and adds it again. The
   changelog and `docs/runner-image.md` say so.
4. **ADR 0089 says so too**, in its amendment of 2026-10-27.
5. **A Kubernetes runner listens nowhere until #3413.** Kubernetes' `RuntimeDefault` refuses
   `pidfd_getfd`, so no server a chat starts there listens, a unix socket's included; the
   operator's pod is not given the runner's profile until #3413 decides how.
