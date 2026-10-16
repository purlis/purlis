### Added

- **The session host starts a chat by its names.** A start that names only its profile,
  persona, workspace, the conversation to carry on, its first message and the person's opt-out
  of the sandbox is composed in the session host from the project's own files and this machine's
  profiles, with the code the window's own start runs. The host starts the chat's sandbox
  confinement itself, and the chat's run in the event log says who started it (`started_by`).
  `purlis serve` composes such starts too, and refuses one that would run sandboxed, with the
  reason, until a runner starts a chat's confinement itself (#2217). (#2171, #2306)
