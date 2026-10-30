### Added

- **A branch folder has one live writer.** The first chat that commits, updates or pushes in a
  branch folder through purlis claims it. When a second live chat tries `purlis worktree commit`,
  `update` or `push` there, purlis refuses and names the chat that holds the folder. The holder
  hands it over with `purlis worktree done` or `abandon`, or the person does. A claim whose
  chat has ended is released. When purlis can't tell whether the holder is still running, for
  example while it restores after a restart, it refuses. A task started in a folder another
  chat holds is warned that its writes there will be refused (#3901).
