### Added

- **A service on this Mac can be allowed by its port.** When a sandboxed chat's command reaches a
  service listening on this machine (`kubectl` against a local cluster at `127.0.0.1:50795`, a
  database in a container, a dev server), the refusal is now a Block whose Notice offers **Allow
  only for this chat** or **Allow for me on this machine**: that one port, never a range or every
  port, and never one of purlis's own. In a Codex or opencode chat the Notice names the port when
  purlis's proxy refused it (a client that uses the proxy, such as curl); for a client that
  connects directly, such as kubectl, its port field is pre-filled from the command's output and
  says to check it. The chat restarts on its conversation to take it, and the grant is listed
  in Settings' Network page with Remove. Codex and opencode chats take it now; a Claude Code
  chat's Notice says its sandbox cannot be given one port alone yet and offers Start without the
  sandbox (qw384).
