### Changed

- **A task starts in auto mode when the chat that asked for it runs in auto.** A Claude Code
  chat in auto mode, or with its permission prompts off, now starts its tasks in auto mode, so
  they no longer stop on tool prompts in tabs nobody is watching. A task never starts with its
  prompts off, and a chat that asks starts tasks that ask. A profile that names its own
  permission mode keeps it. Codex and opencode tasks start in their default mode, as before
  (#3787).
