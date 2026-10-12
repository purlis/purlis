### Changed

- **Removing a branch folder counts the git state only that folder keeps.** `purlis worktree
  remove` and removing a workspace now refuse a folder holding a ref only it has, settings of
  its own (`config.worktree`), or a commit only its history of HEAD reaches, as Tidy and Discard
  already did, and name what would be lost. Where git would not say, the folder is kept too.
  `--force` still discards it (#1472).
