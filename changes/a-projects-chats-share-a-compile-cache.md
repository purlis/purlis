### Added

- **A project's sandboxed chats can share their Rust compiles.** With `compile-cache = "sccache"`
  under `[sandbox]` in its `charter.toml`, each Claude Code chat on macOS builds through the
  `sccache` on your `PATH`. The cache lives in the project's own cache home, so a crate one branch
  folder compiled is not compiled again in the next. Off unless the project turns it on. Each chat
  runs its own sccache server on a socket in its own temp folder, and the chat can reach nothing
  else new (Q324, refs #6388, #3560).
