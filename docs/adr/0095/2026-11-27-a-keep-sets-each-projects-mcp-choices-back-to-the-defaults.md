# Amended 2026-11-27: a keep sets each project's MCP choices back to the defaults

Ruled by the person on 2026-11-27.

The amendment of 2026-11-08 (4) has a keep take every project's own MCP servers out of Claude
Code's state file on the runner, and leave what else a project's entry holds. A project's entry
also holds which MCP servers are on or off for that folder, and the amendment of 2026-10-31 has a
keep restore what a chat may have changed in the profile's settings on a runner. So:

1. **A keep takes each project's MCP choices out of the runner's state file**, at every keep,
   settings sent or not, as it takes out the project's own servers. In Claude Code 2.1.332 those
   are `enabledMcpServers` and `disabledMcpServers`, which it reads for the folder as a chat
   starts, and the older `.mcp.json` approvals `enableAllProjectMcpServers`,
   `enabledMcpjsonServers` and `disabledMcpjsonServers`, which it moves into the folder's
   `.claude/settings.local.json` as it starts. The desktop sends none of them, since its
   projects' entries name its own folders, so each is absent again: Claude Code's default.
   `PROJECT_MCP_CHOICES` in `crates/purlis-core/src/sandbox/claude_wrap.rs` lists them. A
   turned-off server comes back on too, since the default turns none off.
2. **An approval Claude Code has already moved into a folder's settings is not loaded**: every
   Claude Code chat on a runner is sandboxed, and a sandboxed chat does not load a project
   settings file that holds a key outside `harness::claude::project::KEPT` (ADR 0067 as amended
   2026-10-15). The keep does not edit the folder's files.
3. **What else the state file holds stays as it is**: every other key of a project's entry and
   every key beside `projects`.
