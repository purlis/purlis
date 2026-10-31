### Changed

- **A chat keeps out of other harnesses' default folders.** A chat no longer reads or writes
  `~/.claude`, `~/.codex` or opencode's data folder unless they are its own harness's. On a pool,
  Claude Code's and Codex's built-in default profiles stay signed in again after a runner is
  replaced; opencode's signs in again for now.
