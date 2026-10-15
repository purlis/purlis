### Changed

- **Your level `off` now takes effect** (#2071). Where you set or confirmed no sandbox for a
  project, or for a persona, in Settings › Sandbox › Your level, its chats start without the
  sandbox, as "Start without the sandbox" does, and each tab says *no sandbox — your setting*.
  A level written into `purlis.local.toml` any other way waits for your Confirm and changes
  nothing until then. A chat another chat dispatches or hands off to starts at the stricter
  of the two chats' levels, so asking for a chat never turns the sandbox off. Full sandbox is as before, and so is Floor until the floor ships.
