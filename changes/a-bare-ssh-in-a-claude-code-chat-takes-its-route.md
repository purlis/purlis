### Fixed

- **A bare `ssh` in a sandboxed Claude Code chat reaches an allowed host.** Its route through
  the chat's SOCKS port is written where no chat writes, and Claude Code puts it first on each
  command's `PATH` before the command runs, so `ssh` works with the command unchanged, as it
  already did in Codex and opencode chats; a host the chat may not reach is a Block with Allow.
  The harness's own `PATH` never names it (#1667).
