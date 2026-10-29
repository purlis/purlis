### Changed

- **`purlis worktree add` and `update` start from the remote when the clone's branch has moved
  apart from it.** When the clone's `main` and `origin/main` have each moved on, a new piece is
  cut from `origin/main` and the command says so. Before, it was cut from the clone's `main`,
  whose extra commits then showed up in the piece's pull request. `--base HEAD` still cuts from
  the clone as it is (#3836).

### Added

- **`--base origin/main` for `purlis worktree add` and `update`.** A piece can be cut from, or
  moved onto, a remote-tracking branch, so a sandboxed chat, which cannot make or move a branch,
  can always reach the remote. `update --base <branch>` moves a piece onto another base and
  carries only the piece's own commits, as `git rebase --onto` does (#3836).
