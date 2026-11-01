### Added

- **A cluster pool's idle runners suspend after its `idleAfter`** (30 minutes unless the pool says
  otherwise). A runner with no chat, no browser and no project held by its desktop stops and its
  pod ends; a kept runner's volume stays. Its Claim says `Idle`, and the next chat you start there
  runs a kept runner again; a fresh runner comes back with a new key, so its Claim is made again. It takes a runner image with this version of purlis; an older image never
  suspends (#3980).
