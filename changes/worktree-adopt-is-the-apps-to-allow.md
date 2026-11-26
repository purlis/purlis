### Security

- **`purlis worktree adopt` is the app's to allow, not the environment's.** In a session the app
  started, the command asks the app, which runs it only for the window's shell tab, with no
  harness started in it by hand, and refuses it to a chat whatever variables the chat sets. In
  a terminal outside purlis it runs as before (#7700).
