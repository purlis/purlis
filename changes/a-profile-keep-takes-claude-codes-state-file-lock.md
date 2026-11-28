### Fixed

- **A profile keep on a pool's runner no longer loses an update to Claude Code's state file.**
  The keep now takes the lock Claude Code takes for that file, a `.claude.json.lock` folder, and
  reads the file again under it, so a save a running chat makes at the same moment is kept, and
  so is the keep's own reset. If Claude Code holds the lock for more than 3 s, the keep fails
  and says so, and trying again works once the chats have saved. A lock that a stopped Claude
  Code left behind is taken over after 10 s, as Claude Code itself does (#7947).
