### Removed

- **purlis no longer turns other harnesses' plugins on or off per project.** A plugin brings
  hooks, MCP servers, subagents and code as well as skills, and each harness handles those its
  own way, so which plugins load is now the harness's own settings' to say. The Plugins group in
  Settings, the plugin rows in the Skill studio and `[harness_plugins]` are gone. purlis's own
  plugin loads as before on all three harnesses, and `purlis plugin install` stays. The Skill
  studio still lists the skills a plugin brings, read-only, naming the plugin (ADR 0050, #3467).

### Changed

- **Old `[harness_plugins]` keys are ignored, and said once.** A project that still holds them
  starts as any other. One ask in the Inbox names the files, and its Remove shows the change that takes the
  keys out for you to apply, a workspace's `workspace.json` included; nothing is rewritten until
  you do (#3467, #3486). If a file changes before you apply its change, dismissing that stale
  change brings the ask back, with Remove, while the file still holds the keys (#3569).
