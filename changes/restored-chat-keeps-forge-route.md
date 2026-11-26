### Fixed

- **A chat purlis brings back after a restart keeps its forge route.** `purlis worktree push`
  and `update` in a chat the session host kept running were refused with "This chat has no forge
  route" until the chat was started again. The app now gives such a chat a route for its own git,
  through the same sign-in, trust and policy checks as a new chat (#7694).
- **`purlis worktree adopt` works in the window's shell tab.** It took the shell tab for a chat
  and answered "run it from your own terminal". A harness started by hand in a shell tab is still
  refused (#7694).
