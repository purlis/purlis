### Fixed

- **A piece is cut from, and moved onto, the remote when the clone's own branch is stale.**
  `purlis worktree add` and `update` take a remote-tracking branch as `--base` (`origin/main`).
  Where the clone's branch and its remote have each moved on, they use the remote's and say so,
  rather than the clone's. `update` carries only the piece's own commits onto the new base,
  leaving behind those of the base it was cut from (#3836).
