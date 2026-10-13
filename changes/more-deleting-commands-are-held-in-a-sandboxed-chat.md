### Security

- **Holds more deleting commands from a sandboxed chat** (#1408). `purlis persona remove` and
  `purlis worktree remove` are refused there before anything is deleted, as `purlis workspace
  remove` is, with a sentence that asks the person to remove it from the window. A terminal
  removes as before.
