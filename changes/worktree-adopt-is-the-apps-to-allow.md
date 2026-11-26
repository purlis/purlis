### Security

- **`purlis worktree adopt` is the app's to allow, not the environment's.** In a session the app
  started, the command asks the app, which runs it only for the window's shell tab (started as
  your shell, with no harness started in it by hand) and refuses it to a chat. Where the app
  does not take it, nothing is adopted. In a terminal outside purlis it runs as before (#7700).
  What this does not stop: a chat started without the sandbox can leave the app's process tree
  (detach, or start again under `env -i`) and adopt from there, or write purlis's state folder
  directly. A harness started by its absolute path, or one whose start notice is lost, is not
  seen, so the shell tab it runs in still adopts.

### Changed

- **`purlis worktree adopt` in tmux or screen started from the shell tab is now refused.** Their
  server runs outside the app's process tree, so the app cannot tell who asks. Run it in the
  shell tab itself or in your own terminal (#7700).
- **`purlis worktree adopt` in a shell tab that kept running across a relaunch of the window is
  refused.** The new window cannot know whether a harness was started in it by hand before. Run
  it in a new shell tab (#7707).
