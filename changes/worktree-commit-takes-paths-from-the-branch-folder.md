### Fixed

- **`purlis worktree commit` takes paths as git does in the branch folder it runs in.** Run
  inside a branch folder, or with `env -C <branch folder>`, its paths are relative to where it
  runs there, even when the chat was started in the workspace above it. Paths spelled from the
  workspace still work, and a path it cannot place is refused with the spelling that works. A
  message that starts with `--` no longer needs `--message=` (#3927).
