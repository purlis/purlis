# Amended 2026-10-03: purlis's five read-only MCP tools are pre-allowed too (V79, #1050)

The operator's ruling V79 widens the one-allow rule above, for Claude Code only. The same
session `--settings` now carries the Smart close allow and five more, one per read-only tool of
purlis's own MCP server (HP-7, `chattools::PRE_ALLOWED`):

`mcp__charter__todo_list`, `mcp__charter__memory_search`, `mcp__charter__session_record_list`,
`mcp__charter__session_record_read` and `mcp__charter__change_status`.

- **Each tool by its full name, never the server.** An `mcp__charter` rule would allow every
  tool purlis's server offers, including one added later.
- **The writes and `ask_operator` still prompt.** `todo_add`, `todo_done` and `memory_add` change
  the workspace. `ask_operator` is marked read-only, but it is a question for the operator and
  goes through the harness's prompt anyway. The list is written out, not derived from the
  read-only mark, so a new tool asks until someone rules it in.
- **Still only `allow`, still a session flag.** No `ask`, `deny` or mode is added, nothing is
  written to a settings file, and an operator's own `ask` or `deny` for any of these tools still
  wins, as measured above for the record command.

**A rule names a server by its name, so which server answers to `purlis` was measured.** Claude
Code also loads MCP servers from a project's `.mcp.json`, from the local and user scopes and from
the operator's own configuration, and a server registered there under the same name would answer
to the same tool names, pre-allows included. Measured on Claude Code 2.1.288, one launch per
case: the real `claude -p` with a scratch `HOME` and `CLAUDE_CONFIG_DIR`, a dummy token and an
unreachable API, reading the server list and the tool list it reports at start. Two stand-in
stdio servers both named `charter` offered different tools, one handed over with `--mcp-config`
the way purlis hands it, and one in the scope under test.

| The other `purlis` | Without `--mcp-config` | With purlis's `--mcp-config` |
|---|---|---|
| project `.mcp.json`, not approved | not loaded | the session's answers; the other is never started |
| project `.mcp.json`, approved by name | it answers | the session's answers; the other is never started |
| project `.mcp.json`, every project server enabled | it answers | the session's answers; the other is never started |
| local scope | it answers | the session's answers; the other is never started |
| user scope | it answers | the session's answers; the other is never started |

The session's `--mcp-config` server wins in every scope, and the same-named one is not started
at all. So the name stays `purlis`, without `--strict-mcp-config`, which would also drop the
operator's own servers from every chat. It is what 2.1.288 does, not a rule Claude Code
documents: one whose precedence changed would hand these five allows to another server, so it is
measured again with this ADR's other Claude Code measurements. A session-unique server name was
considered and not taken: it would change every tool's name in every chat for a case that
measured as closed.

Codex and opencode are not covered by this ruling, and nothing is added for them.
