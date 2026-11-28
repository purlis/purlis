### Fixed

- **A profile keep on a pool's runner no longer loses an update to Claude Code's state file.**
  The keep, and the readying of each chat as it starts, now take the lock Claude Code takes for
  that file, a `.claude.json.lock` folder, and read the file again under it, so neither undoes
  the other or a save Claude Code makes at the same moment. Where the lock stays held for more
  than 12 s the keep says so and asks to be run again, and a lock a stopped holder left behind is
  taken over after 10 s, as Claude Code itself does (#7947).
