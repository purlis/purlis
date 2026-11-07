### Fixed

- **A clone a runner's chat made comes back into `repos/`.** Moving a chat back from a runner now
  makes its new clone at `workspaces/<ws>/repos/<name>`, like every other new clone, and a folder
  of yours with that name at the workspace's root no longer stops it. `purlis clone repos` finds
  an old clone named `repos` instead of refusing it, and a clone that fails no longer leaves an
  empty `repos/` folder behind (#5534).
