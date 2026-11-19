### Fixed

- **A dead Bash guard refuses under `open` where your profile's own rules name a program.** The
  start read your Claude Code settings from the app's folder, not from the `CLAUDE_CONFIG_DIR` the
  chat's profile sets, so a deny rule kept only there let a dead guard through. Inline `--settings`
  that purlis cannot read are now refused for a chat another chat starts (#7155).
