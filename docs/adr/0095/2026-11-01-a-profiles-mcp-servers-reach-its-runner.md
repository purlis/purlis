# Amended 2026-11-01: a profile's MCP servers reach its runner

**Built by #3846**, under spec #2041. It extends item 1 of the 2026-10-29 amendment, whose
ownership rule stands: only a runner shown to be the person's own is sent any. Everything here is
**decided in implementation, to be confirmed** (D-3846-1 to D-3846-4 in the pull request).

1. **Only the servers leave the state file** (session protocol 1.66). Claude Code keeps user-scope
   MCP servers in `.claude.json`, among sign-in and project records. The desktop takes `mcpServers`
   alone and sends it as a settings file, `mcp-servers.json`, filtered as any settings file is.
2. **The runner puts them into its own state file**, which only purlis writes, in place of the
   servers it held. What else that file holds stays. No servers sent is a list too, so a server taken
   out on the desktop goes there.
3. **A runner before 1.66 is sent none**, since it refuses a settings file it does not know, and
   with it all the others.
4. **Plugins are not sent** (#4441). They are not settings files: a real plugin cache is past a pool's
   default limit, and what lists them holds this machine's absolute paths.
