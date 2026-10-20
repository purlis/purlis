### Fixed

- **Every check that tells a sandboxed chat from one without the sandbox goes by how the chat's
  run started.** Dispatching to another persona, a handoff, a dispatch while you are away, the
  Network view, the app's git actions and the vault Notice read the session host's record of how
  the run started. On each check, a chat with no such record is treated the way that allows it
  less. A chat on a runner is judged by the runner's record: one that runs there without the
  sandbox keeps its persona's browser logins, and one in the runner's sandbox is told why it has
  none, instead of being offered a restart that would not help (#2692).
