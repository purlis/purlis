### Fixed

- **A chat no longer stops on a permission prompt around its own tasks.** A call whose every
  dispatch only looks (`list`, `read`, `wait`, `--help`) is left to the harness's own rules
  whatever stands beside it, so `purlis dispatch list 2>&1 | head -20` no longer asks, unless a
  redirection on the line writes a file. A dispatch that writes, a handoff or a worktree commit
  that shares its call with another command is now refused with the shape to retry ("run the
  dispatch in a call of its own") instead of asking the person. `purlis dispatch close` no longer
  prompts in a Claude Code chat, and `purlis dispatch cancel` is refused for a task the person
  has typed in, as `close` already was (#3785).
