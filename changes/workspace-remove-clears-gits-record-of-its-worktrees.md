### Fixed

- **`workspace remove` clears git's record of each linked worktree it held.** A workspace
  holding a checkout of a repository kept elsewhere left that repository listing the deleted
  folder as a worktree until someone pruned it. Now each such folder's record is removed with
  `git worktree remove`, one folder at a time and never a blanket prune. Where git refuses, a
  locked worktree for one, the removal says which `git -C <repository> worktree prune` clears it
  (#998).
