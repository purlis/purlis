### Fixed

- **A branch folder whose commits merged in a pull request now counts as finished.** When a pull
  request got more commits after a folder's tip and then squash-merged, removing the folder
  was refused, listing commits "that exist nowhere else", although they were part of what
  merged. Now `purlis worktree remove` and `purlis worktree prune` ask the forge. If the
  newest pull request on the branch merged and its head holds the folder's tip, the folder goes
  without `--force`, and its branch with it. If the forge cannot be asked, for example
  offline, the folder stays and the refusal says why (#5439).
- **A branch whose folder is already gone is no longer left behind.** `purlis worktree prune`
  now also deletes each branch purlis cut whose folder was removed some other way, when the
  branch is on its base, carried there by a squash merge, or merged in a pull request. It keeps
  every other branch and says why. `purlis worktree remove --delete-branch` on such a folder deletes the
  branch, or says why it stays, instead of a sentence pointing at a folder that is gone. From a
  chat the app deletes the branch, so its section of the clone's git config goes too (#5439).
