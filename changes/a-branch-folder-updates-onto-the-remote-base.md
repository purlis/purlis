### Fixed

- **A branch folder is cut from and updated onto the remote's base where the clone's own has
  diverged.** Where the clone's `main` holds commits its remote never took and the remote has
  moved on too, `purlis worktree add` and `purlis worktree update` start from `origin/main`
  instead, and leave the clone's `main` where it is (#3890).
