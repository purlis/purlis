### Added

- **purlis brings a sandboxed chat's clone, branch folder or project root up to date for
  it.** `purlis worktree update` now works in a workspace's clone as it did in a branch
  folder. It fetches, then fast-forwards or rebases, with `--continue` and `--abort` after a
  conflict. At the project root it runs the window's own Update for the chat. In a chat the
  app runs it without asking, from the command or the new `worktree_update` tool. It refuses
  uncommitted changes and any move that would overwrite a file that is not committed. A move
  that would write a file the chat's sandbox denies is still left to the person. The refusals
  that sent a chat's `git pull` to the person now name the command (#3831, #3862).
