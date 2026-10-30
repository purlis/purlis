### Fixed

- **Claude Code chats start on a pool's runner.** A sandboxed Claude Code chat on a Docker or
  cluster pool was refused for want of bubblewrap and socat. It now runs inside purlis's own Linux
  sandbox there, as Codex and opencode chats do, with Claude Code's own sandbox off, and the runner
  image needs neither program. Its commands can read the runner's sign-in, as Codex's can. On a
  runner a Claude Code chat always starts sandboxed: "Start without the sandbox" is not offered
  there, and such a start is refused. On your own machine nothing changes (#4083).
