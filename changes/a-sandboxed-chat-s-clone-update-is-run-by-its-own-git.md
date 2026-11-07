### Changed

- `purlis worktree update` standing in a workspace's clone, for a sandboxed chat, no longer
  rebases there in the app. The app fetches, works out the rebase and names the exact `git`
  command for the chat to run in the clone; `purlis worktree update --continue`, or the update
  run again, then puts the branch on the result. `--abort` before the named step has run puts
  the folder back as it was. Branch folders, chats that are not sandboxed and the terminal are
  unchanged.
