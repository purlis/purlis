### Added

- **Stricter terminals**, a project's sandbox setting in Settings › Sandbox, off by default
  (`own-terminal-only = true` under `[sandbox]`). On a Mac it keeps a sandboxed Codex or opencode
  chat to the terminal it runs in, so it can't read or type into your other terminal tabs or
  another chat's terminal. While it's on, a chat can't make a terminal of its own, so `script`,
  `expect`, `tmux` and Codex commands that run in a terminal fail; the setting says so.
