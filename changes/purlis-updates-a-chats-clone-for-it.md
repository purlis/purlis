### Added

- **purlis brings a sandboxed chat's clone up to date for it.** `purlis worktree update`, run
  in a workspace's clone, fetches and fast-forwards or rebases it, with `--continue` and
  `--abort` after a conflict, as it already did for a branch folder. In a chat the app runs it
  without asking, from the command or the new `worktree_update` tool. It refuses uncommitted
  changes and any move that would overwrite a file that is not committed. The refusals that
  sent a chat's `git pull` to the person now name it (#3831).
