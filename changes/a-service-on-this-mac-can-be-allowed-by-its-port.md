### Added

- **A service on this Mac can be allowed by its port.** When a sandboxed chat's command reaches a
  service listening on this machine (`kubectl` against a local cluster at `127.0.0.1:50795`, a
  database in a container, a dev server), the refusal is now a Block whose Notice names the port
  and offers **Allow only for this chat** or **Allow for me on this machine**: that one port,
  never a range or every port, and never one of purlis's own. A port the refusal did not name is
  typed in the Notice. The chat restarts on its conversation to take it, and the grant is listed
  in Settings' Network page with Remove. Codex and opencode chats take it now; a Claude Code
  chat's Notice says its sandbox cannot be given one port alone yet and offers Start without the
  sandbox (qw384).
