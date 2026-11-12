### Changed

- **A new project starts its chats at Basic sandbox, your own default.** `purlis init` and the
  app no longer write `mode = "on"` into a new project's `charter.toml`: its `[sandbox]` block
  names the default egress and no level, so its chats start at the level you set on this
  machine, Basic sandbox unless you chose another. The project offers the full sandbox once, as
  any project at Basic sandbox does. A project made before keeps the `mode` it has.
