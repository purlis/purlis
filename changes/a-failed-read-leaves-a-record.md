### Fixed

- **A tool call that failed now leaves its line on the record, whichever tool it was.** The failure
  hook was heard on a failed shell command alone, so a failed `Read`, `Write`, `Edit`, `Grep`,
  `Glob`, `WebFetch`, `Skill`, `Task` or `Agent`, or a failed call of another MCP server's tool,
  left nothing behind. A sandbox block is still read of a failed shell command alone (#6888).
