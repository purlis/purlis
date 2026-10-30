### Fixed

- **Claude Code chats start on a pool's runner.** A sandboxed Claude Code chat on a Docker or
  cluster pool was refused for want of bubblewrap and socat. It now runs inside purlis's own Linux
  sandbox there, as Codex and opencode chats do, with Claude Code's own sandbox off, and the runner
  image needs neither program. Its commands can read the runner's sign-in, as Codex's can. On your
  own Linux machine nothing changes (#4083).
