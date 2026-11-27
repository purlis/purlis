### Fixed

- **A chat's `purlis save --pull` in a project that saves through a pull request now sends the
  person to Update in the window**, where it used to tell the chat to run `purlis worktree
  update`, which that mode refuses. The refusal also reaches the person's Inbox.
