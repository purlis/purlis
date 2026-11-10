### Fixed

- **On a Linux desktop, Pool setup's Try it and a chat you start on a pool use the pool's own
  Claude Code profile.** They are no longer refused for want of a sandbox-only profile on your
  machine: the pool's runner runs every sandboxed Claude Code chat inside purlis's sandbox, on
  the profile its Sign-ins step kept (#2446, ADR 0067 as amended 2026-11-10).
