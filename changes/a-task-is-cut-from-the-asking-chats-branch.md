### Changed

- **A task asked for from a chat that works in a worktree is cut from that chat's branch.** With
  `--in worktree` (or a persona's `dispatch-isolation: worktree`), the task's new branch now
  starts from the branch the asking chat's own worktree is on, so the task sees its commits, and
  Merge lands it back into that branch, in that worktree. A chat in the clone itself gets a task
  cut from the clone's current commit, as before (#1472).
- **A Discard refused for a change says what changed.** It names the paths new to, or gone from,
  what would be lost, a commit made, or a folder switched to another branch. Where nothing new
  would be lost and a file was only written again, or a file went away while purlis read the
  folder, it says something is still writing there and to stop it first (#1472).

### Fixed

- **A worktree cut for a task by an app that quit before the task started is listed.** The
  task's record is now written before its worktree is cut, so the project's next open settles it
  and takes away a worktree nothing was written in (#1472).
