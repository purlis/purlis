### Changed

- **Whether anyone is at a chat is read from one fact everywhere.** Publishing, dispatching, a
  chat's start briefing and the "Commitment point" nudge now all go by the attendance purlis
  holds for the chat, never by the permission mode a hook payload names. A Codex chat you
  started is no longer treated as one nobody is at because Codex reports `bypassPermissions` on
  every line: it can dispatch as an attended chat, and its briefing no longer tells it to stop.
  A chat purlis started whose app doesn't answer is treated as unattended. In Claude Code, a
  line in `bypassPermissions` is still refused a publish.
