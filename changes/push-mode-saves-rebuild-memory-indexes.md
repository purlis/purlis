### Changed

- **A save in `push` mode rebuilds memory indexes too.** Its replay merges each of this
  machine's commits as a request's merge does, so a memory store's `MEMORY.md`, the session
  indexes and the todos index are written again from the files the replay left, and
  `charter.toml` merges key by key. A new plane's `.gitattributes` no longer holds union rules
  for memory indexes; a plane's existing block keeps them (#5864).
