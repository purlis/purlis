### Fixed

- **A renamed workspace's memory header and Log line name it by its new name.** `purlis
  workspace rename` now also rewrites the `-w <name>` purlis wrote into the workspace's
  `memory/MEMORY.md` header and the Log line of its `workspace.md`, so a chat that follows them
  reaches the workspace instead of being told there is none. Lines a person wrote are left as
  they are (#3965).
