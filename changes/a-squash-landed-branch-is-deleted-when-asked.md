### Changed

- **A squash-landed branch goes when its folder does.** `purlis worktree remove --delete-branch`
  and a task's Discard now delete a branch whose change landed by a squash merge, where before
  git's own check kept it because it does not find a squashed branch merged. Only the local
  branch goes, and a branch whose base has changed its files since stays (ADR 0110, #5168).
