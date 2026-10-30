### Added

- **`--in worktree:<repo>` gives a task a branch of its own from a workspace whose clones sit in
  subfolders.** A chat in a workspace's own folder used to be refused `--in worktree`, so its
  tasks shared one checkout. Now `--in worktree` uses the workspace's clone where it holds one.
  Where it holds several, the refusal names them, and `--in worktree:<repo>` (the `dispatch`
  tool's `in: worktree:<repo>`) picks one. The task's branch folder is cut the way `purlis
  worktree add` cuts one, from the remote's default branch as the clone last synced it, and is
  claimed by the task. The task
  then commits, updates and pushes with `purlis worktree commit`, `update` and `push`. The "no
  file locks between tasks" warning names this way out, and a Claude Code chat runs a dispatch
  line that starts with `--in` without asking (#3903).
