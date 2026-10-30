### Added

- **purlis removes a chat's finished branch folder for it when nothing would be lost.** In a
  chat, `purlis worktree remove <repo> <piece>` and the new `worktree_remove` tool are run by
  the app, without asking, for a branch folder of the chat's own workspace. The folder goes only
  when it holds no uncommitted change and no commit that no other branch reaches; otherwise the
  answer names what would be lost and nothing is removed. `--force` stays the person's, from
  the window (#3879).
