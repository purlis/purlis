### Fixed

- **A profile kept on a pool's runner sets each folder's MCP server choices back to their
  defaults.** Every keep already takes the MCP servers a chat added to a folder out of Claude
  Code's state file on the runner. It now also takes out which servers a chat turned on or off
  for that folder, and the older `.mcp.json` approvals, so a later chat there starts with what the
  profile says. Nothing else in that file changes.
