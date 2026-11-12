### Changed

- **A new project starts its chats at Basic sandbox, your own default.** `purlis init` and the
  app no longer write `mode = "on"` into a new project's `charter.toml`: its `[sandbox]` block
  names the default egress and no level, so its chats start at the level you set on this
  machine, Basic sandbox unless you chose another. To run a new project in the full sandbox,
  turn it on in Settings; a new project is not offered it, as one made before the sandbox
  existed still is. A project made before keeps the `mode` it has. Where Basic sandbox can't be
  applied (Codex or opencode on Linux and other systems that are not macOS, or a system with no
  sandbox backend), a new project's chats start with no sandbox, and each chat's pane says so.
