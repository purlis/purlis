### Fixed

- **The Chat log's start row names the plane's own file.** A project whose file is
  `purlis.toml` now has its commit recorded, where it was always said to be in no commit, and
  the row says `charter.toml` for a project that still has that one.
- **The notice that a chat runs in a mode purlis didn't set names the chat's own harness**, and
  its mode in that harness's words, never Claude Code's for a chat on another harness.
- **An unattended chat with no workspace is told so in words true of every harness**, no longer
  as Claude Code's `bypassPermissions` mode.
