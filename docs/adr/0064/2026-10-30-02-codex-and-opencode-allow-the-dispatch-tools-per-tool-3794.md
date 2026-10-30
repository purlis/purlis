# Amended 2026-10-30: Codex and opencode allow the dispatch tools per tool (#3794)

Follows the amendment of 2026-10-29 (#3786), which left Codex and opencode to their own approval
rules for the eleven dispatch tools. On those harnesses a chat could still stop on a prompt for
its own tasks. Consent to a dispatch is purlis's own dispatch grant (V98b), so each harness now
runs these tools unasked, by name, as Claude Code does.

- **One neutral list.** `chattools::run_on_the_dispatch_grant` names the eleven tools:
  `DISPATCH_TOOLS` and the three that only read a chat's own tasks. Each adapter turns the list
  into its own rule, as `browsertools::run_on_the_browser_ask` does for the browser tools.
- **Codex.** The `-c mcp_servers.purlis={…}` the chat is started with carries
  `tools.<tool>.approval_mode = "approve"` for each. Measured on codex-cli 0.163.0: the key is
  read under one MCP server, and `auto`, `prompt`, `writes` and `approve` are the only words it
  takes there. Never the server's `default_tools_approval_mode`, so a tool added later still
  asks.
- **opencode.** The session config in `OPENCODE_CONFIG_CONTENT` carries
  `permission.purlis_<tool> = "allow"` for each. Measured on 1.18.33: an MCP tool is listed and
  asked about under `<server>_<tool>` with pattern `*`, and `opencode debug agent build` shows
  each rule as `purlis_<tool> allow *`. opencode applies the last rule that matches, and this
  layer is merged after the project's. So an operator's rule under the very same name is
  replaced, while a rule under a pattern of its own (`purlis_*`) keeps its place in the list.
  This differs from Claude Code, where an operator's `ask` or `deny` always wins. The dispatch
  grant stands in front of every dispatch either way.
- **Not in this amendment.** The other read-only, own-scope and browser tools that Claude Code
  allows are still decided by Codex's and opencode's own rules. #4130 brings them
  across the same way.
