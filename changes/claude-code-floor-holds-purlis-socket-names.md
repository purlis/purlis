### Security

- **A Claude Code chat at the floor can no longer move or link purlis's sockets aside.** Its settings now hold `hooks.sock`, `slots.sock`, `purlisd*.sock`, `purlis-crash-*.sock`, the app's single-instance socket and the `purlis-serve` and `engine-links` folders from being written, as the Codex and opencode wraps do, whatever `ports` says (round 6 of #7105).
