### Changed

- **A chat's git write that the sandbox blocks in a branch folder no longer raises a Notice for
  you.** When a chat's `git add` or `git commit` in its branch folder slips past the guard and
  meets the sandbox, the chat alone is told to retry with `purlis worktree commit`. The block
  is still kept in the network record (#3861).
