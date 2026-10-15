### Added

- **`sandbox.proposed-changes` in this machine's policy file.** `"off"` turns proposed changes
  off: a chat's change to purlis's own configuration (the project's manifest, a persona's
  `mcp.json` and `persona.md`, a `.claude/skills` file) is refused as before and never shown
  to you to apply. `"on"`, or no key, leaves them on. Any other value refuses the policy file.
  The window that shows a proposed change comes next (#1975).
