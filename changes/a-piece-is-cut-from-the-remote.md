### Added

- **`--base origin/main` for `purlis worktree add` and `update`.** A piece can be cut from, or
  moved onto, a remote-tracking branch, so a sandboxed chat, which cannot make or move a branch,
  can always reach the remote. `update --base <branch>` moves a piece onto another base and
  carries only the piece's own commits, as `git rebase --onto` does, and records the new base
  (#3836).

### Fixed

- **An update leaves behind the commits of the base a piece was cut from that its new base
  lacks.** A piece cut from a clone's `main` that held commits its remote never took no longer
  carries them onto `origin/main` when it is updated: only its own commits move (#3836).
