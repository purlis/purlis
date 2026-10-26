### Added

- **A sandboxed chat may listen on a port.** When a server a Codex or opencode chat starts is
  refused its port (`npm run dev` on `127.0.0.1:5173`), the refusal is a Block whose Notice
  offers **Allow only for this chat** or **Allow for me on this machine**: that one port on this
  Mac only (127.0.0.1), never another interface, a range or one of purlis's own. The chat restarts on
  its conversation, is told to start the server on `127.0.0.1:<port>`, and its browser opens
  `http://127.0.0.1:<port>` with no second ask. A port nobody allowed stays refused to the chat
  and its browser. The grant is listed in Settings' Network page with Remove. On Linux the wrap's
  supervisor makes the bind itself. A Claude Code chat's sandbox cannot be given one port to
  listen on, so its Notice says so and offers Start without the sandbox (#2019).
