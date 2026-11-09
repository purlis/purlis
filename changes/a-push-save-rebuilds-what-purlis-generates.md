### Fixed

- **A save in `push` mode no longer stops on a memory index both machines added to.** Its
  replay rebuilds a memory store's `MEMORY.md`, a `sessions/index.md` and a workspace's
  `todos/MEMORY.md` from their sources, as every merge purlis makes already did, and merges the
  settings file key by key, so only what a person wrote can stop it. A new project's
  `.gitattributes` no longer carries the two memory `merge=union` rules; an existing project
  keeps them until `reinit` (#5864).
