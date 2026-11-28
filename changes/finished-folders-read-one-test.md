### Fixed

- **A finished branch folder goes the same way from everywhere.** Closing a task's chat, opening
  a project, the explorer's Remove, `purlis worktree remove` and `purlis worktree prune` now ask
  one test of whether a folder is finished. A task's folder whose pull request merged, or that
  only holds what a build wrote (`target/`, `node_modules/`, `dist/`), is taken away when its
  chat closes. The explorer's Remove takes a folder that a squash merge carried, or that sits
  inside a merged pull request's head, without asking for force. A package's `logs/` no longer
  keeps a folder (#5476, #7406). A folder another open chat works in is kept, and so is every
  folder of a workspace whose claims or links purlis cannot read.
