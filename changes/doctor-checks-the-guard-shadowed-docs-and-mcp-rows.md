### Added

- **`purlis doctor` checks the Bash guard, shadowed docs and MCP rows.** Four rows that said
  "not checked" now look:
  - `plane-root guard` says whether the Bash guard is declared where a session started in this
    directory loads it: its own `.claude/settings.json` or `settings.local.json`, the user
    settings of the Claude Code folder in use, or an enabled plugin. A guard declared twice is
    named. The chats the app starts are armed by the app either way, so a guard declared
    nowhere is a warning about chats started outside it.
  - `guard seen` says when the guard last ran in this project and under which harness. The
    guard records each run in `guard-seen.json` in the project's state folder.
  - `shadowed docs` names a project's own copy of a page `purlis docs show` serves, or of a
    skill purlis's plugin ships, which drifts unwatched.
  - `mcp` says whether each MCP server a persona's chats are given can start: a launcher on an
    absolute path that is not there is named, with its persona (#994).
