### Added

- **purlis removes a chat's finished branch folder for it when nothing would be lost.** In a
  chat, `purlis worktree remove <repo> <piece>` and the new `worktree_remove` tool are run by
  the app, without asking, for a branch folder of the chat's own workspace. The folder goes only
  when it holds no uncommitted change and no commit that no other branch reaches, or when its
  branch landed by a squash merge, here or on the remote; otherwise the answer names what would
  be lost and nothing is removed. A folder another live chat holds is not removed for it, and a
  removal ends the folder's claim. `--force` stays the person's, from the window (#3879, #3969).

### Security

- **A chat removes a branch folder of its own workspace without a prompt** (#3879). Claude Code
  chats are now allowed `purlis worktree remove` and the `worktree_remove` tool unasked, alone
  in their call. The app never forces the removal, refuses a folder another live chat holds,
  and `--force` is refused for every chat, by the command and by purlis's hook.
