### Changed

- **Your level `off` now takes effect** (#2071). Where you set or confirmed no sandbox for a
  project, or for a persona, in Settings › Sandbox › Your level, its chats start without the
  sandbox, as "Start without the sandbox" does, and each tab says *no sandbox — your setting*.
  A level written into `purlis.local.toml` any other way waits for your Confirm and changes
  nothing until then. A chat another chat dispatches or hands off to starts at the strictest
  level across the chain of chats that asked, so asking for a chat never turns the sandbox off. Elsewhere chats start as before: at the floor in a project that has not turned the full
  sandbox on, and in the full sandbox in one that has, whether you picked Full sandbox or Floor.
