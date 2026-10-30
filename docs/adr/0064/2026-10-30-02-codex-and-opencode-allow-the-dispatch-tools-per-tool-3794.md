# Amended 2026-10-30: Codex and opencode allow the dispatch tools per tool (#3794)

Follows the amendment of 2026-10-29 (#3786), which left Codex and opencode to their own approval
rules for the eleven dispatch tools. On those harnesses a chat could still stop on a prompt for
its own tasks. Consent to a dispatch is purlis's own dispatch grant (V98b), so each harness now
runs these tools unasked, by name, as Claude Code does.

- **One neutral list.** `chattools::run_on_the_dispatch_grant` names the eleven tools:
  `DISPATCH_TOOLS` and the three that only read a chat's own tasks. Each adapter turns the list
  into its own rule, as `browsertools::run_on_the_browser_ask` does for the browser tools.
- **The operator's own rules always win**, as their `ask` or `deny` does in Claude Code. Each
  harness merges purlis's layer for the session after the operator's, so each adapter reads the
  operator's config first and leaves alone what it names.
- **Codex.** The `-c mcp_servers.purlis={…}` the chat is started with carries
  `tools.<tool>.approval_mode = "approve"` for each tool. That `-c` replaces the operator's
  table for the server whole, so the approvals they set on it in `$CODEX_HOME/config.toml` and
  in any `.codex/config.toml` from the root down to the chat's folder are carried into it, and
  win: a tool's own `approval_mode` is kept, and where they set the server's
  `default_tools_approval_mode` it is kept and no tool is approved. A config purlis cannot read
  approves nothing. Never the server's default of purlis's own, so a tool added later still
  asks.
- **Codex, measured as far as config.** On codex-cli 0.163.0 the key is read under one MCP
  server, and `auto`, `prompt`, `writes` and `approve` are the only words it takes there.
  **Accepted by config, not measured live**: a call under `approval_policy` `untrusted` or
  `on-request` needs a model server, and the machine this was built on cannot bind one.
- **opencode.** The session config in `OPENCODE_CONFIG_CONTENT` carries
  `permission.purlis_<tool> = "allow"` for each tool. Measured on 1.18.33: an MCP tool is listed
  and asked about under `<server>_<tool>` with pattern `*`, and `opencode debug agent build`
  shows each rule as `purlis_<tool> allow *`. opencode applies the last rule that matches, and
  this layer is merged after the operator's, so an allow here would beat their earlier
  `purlis_*` or `*` rule. So purlis first reads the operator's layers: the global config folder,
  `~/.opencode`, `$OPENCODE_CONFIG`, `$OPENCODE_CONFIG_DIR`, and each folder from the chat's up
  to the root with its `.opencode`, in each the top-level `permission` and each agent's, in JSON
  and in agents' Markdown. A tool any of their rules names, other than a bare `*`, is left to
  them. A bare `*` that asks or denies is their word for every tool, and nothing is allowed. A
  file purlis cannot read allows nothing.
- **Not in this amendment.** The other read-only, own-scope and browser tools that Claude Code
  allows are still decided by Codex's and opencode's own rules. #4130 brings them
  across the same way.
