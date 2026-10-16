# Amended 2026-10-16: a chat's browser tools run on its browser Ask (#2245)

Ruled by the operator on 2026-10-16 (#2245): once the person has answered a chat's browser Ask,
that answer is the consent for the chat's browser tools, and they run without the harness's own
per-tool prompt, as the dispatch tools do (V98b, above). A Claude Code chat the app starts
carries an `allow` for each of purlis's browser tools (#2013), `mcp__purlis__browser_open`
through `mcp__purlis__browser_close`, on the rules above: an `allow` only, each by its full
name, and an operator's `ask` or `deny` still wins, as does a persona's `disallowed-tools:`.

- **The consent is the browser Ask, which the app holds.** Every call of a browser tool reaches
  the app on the chat's hook channel, and the app answers it from its own record of the chat
  (`chatbrowsers`). A chat's first call asks the person what its browser may reach (this
  project's sites and its dev servers, also the open web, or Keep off) and opens nothing; every
  call while the Ask waits is refused the same way; Keep off refuses every call. Only after the
  person allows it does a browser start, so the harness's prompt beside the Ask asked twice for
  one thing.
- **Where a browser goes is still the reach proxy's to decide.** The Ask sets a browser's
  posture; the session host's reach proxy enforces what each page may reach, and the person's
  answers to a page that needs them stay Asks of their own. Nothing here widens a browser's
  reach.
- **The rules are static for the chat, and handed only where the chat may have a browser.**
  They ride on the chat's `--settings`, on the same arguments as the `--mcp-config` that hands
  it purlis's server, the one server that serves the browser tools. A chat that was handed no
  server is handed no rules. A chat the window does not have open is refused every browser
  call by the app, whatever its harness allows.
- **Each tool by name, never the prefix.** `mcp__purlis__browser_*` would allow a browser tool
  added later before anyone ruled on it. The list is `browsertools::TOOLS`, the tools the
  server serves, and a test holds it by name.
- **Claude Code only.** Codex is handed nothing for them, as for the dispatch tools: purlis
  arms no per-tool approval there until it is measured on the version purlis supports.
  opencode stays the documented gap above: a per-session pattern would outrank an operator's
  own rule.
