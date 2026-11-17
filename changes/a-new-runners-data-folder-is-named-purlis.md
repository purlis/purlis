### Changed

- **A new pool runner keeps its projects under `~/.local/share/purlis`**, no longer under the old
  `charter` name. A runner that already has its `charter` folder keeps it, since its projects'
  git worktrees and its chats' sessions point at it.
