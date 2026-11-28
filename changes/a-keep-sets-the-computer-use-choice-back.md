### Fixed

- **A profile kept on a pool's runner also sets Claude Code's computer-use choice back to its
  default.** A later chat on that runner is asked again, as it would be on a fresh one. Nothing
  else in Claude Code's state file changes.
