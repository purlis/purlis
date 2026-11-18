### Fixed

- **The dead Bash guard's start reads the rules your chat's own hook reads.** A profile's `HOME`
  and a relative `CLAUDE_CONFIG_DIR` (read from the chat's folder) now count, and an empty
  `CLAUDE_CONFIG_DIR` no longer falls back to the app's. A chat another chat starts is also
  refused, or has them taken out, where its inline `--settings` allow all of Bash, carry `hooks`
  or set `disableAllHooks`; unreadable inline settings are refused with a reason that says so
  (#7165).
