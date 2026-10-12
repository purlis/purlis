### Changed

- **Removing a branch folder counts the git state only that folder keeps.** `purlis worktree
  remove` and removing a workspace now refuse a folder holding a ref only it has or settings of
  its own (`config.worktree`), and name what would be lost. A commit that was amended or
  rebased away does not stop a removal. Where git would not say, the folder is kept too.
  `--force` still discards it (#1472).
