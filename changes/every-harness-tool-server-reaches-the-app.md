### Fixed

- **purlis's chat tools that the app answers work in Codex and ACP chats too.** `dispatch`,
  `dispatch_list`, `dispatch_report`, `persona_where`, `persona_remember`, `session_record` and
  the `workspace.md` tools reach the app that started the chat from every harness, as they did
  from Claude Code and opencode. The tool server is handed the chat's own connection to the app,
  the one its hooks already use, and nothing more (#1996).

### Changed

- **`persona_where`'s answer may be larger than one line.** The chat's hook socket speaks a
  framed stream beside its lines, chosen by the client at the start of a connection, which can
  also carry bytes for tools that come later; hooks and commands that speak lines are unaffected
  (#1996).
