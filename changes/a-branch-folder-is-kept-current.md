### Added

- **`purlis worktree update` and `purlis worktree push`.** From inside a branch folder, `update`
  fetches and rebases it onto the branch it was cut from (or `--base <branch>`), stopping on a
  conflict with the files named, then `--continue` or `--abort`; `push` pushes the folder's own
  branch, with `--force-with-lease` right after an update. In a sandboxed chat the app runs
  them, as it runs `purlis worktree commit` (qw387).

### Changed

- **`purlis worktree add` cuts from the remote's default branch** (`main`, most often), brought
  up to the remote where nothing has it checked out, rather than from whatever the clone is on.
  `--base <branch>` names another branch; `--base HEAD` keeps the clone's HEAD (qw387).
