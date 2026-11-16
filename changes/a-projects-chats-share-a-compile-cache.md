### Added

- **A project's sandboxed chats can share their Rust compiles.** With `compile-cache = "sccache"`
  under `[sandbox]` in its `charter.toml`, each Claude Code chat on macOS builds through the
  `sccache` on your `PATH`. The cache lives in the project's own cache home, one per persona, so a
  crate one branch folder compiled is not compiled again in the next. Off unless the project turns
  it on. Each chat's sandboxed commands run their own sccache server on a socket in the chat's own
  temp folder, with sccache's config pinned to an empty file of purlis's. The chat can reach
  nothing else new (Q324, refs #6388, #3560).
