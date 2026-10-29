### Changed

- **An approved MCP server's environment values come from the person's approval.** In a
  sandboxed chat, a persona's credentialed MCP server approved with `purlis persona approve-mcp`
  starts with each variable its entry's `env` writes out, at the value its approved line shows.
  A value the harness fills in (`${VAR}`) is not on the line, so the server starts without it,
  and `approve-mcp` says so under the line, with what to write instead (#3441).
