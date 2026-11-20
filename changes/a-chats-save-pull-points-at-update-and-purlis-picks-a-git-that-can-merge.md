### Fixed

- **A chat's `purlis save --pull` in a project that saves through a pull request now sends the
  person to Update in the window**, where it used to tell the chat to run `purlis worktree
  update`, which that mode refuses. The refusal also reaches the person's Inbox.
- **purlis runs a git that can merge, not just the first one it finds.** On a Mac whose Apple
  git is older than 2.40, a newer Homebrew git is now chosen, so a save that replays commits no
  longer fails with git's usage text (#6846). With no git of 2.40 or later, the message names
  the git found, its version and the version needed.
