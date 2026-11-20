### Changed

- **Two desktops can share one Docker engine.** A pool's runner now has a container and home
  volume named for the desktop that made it (`purlis-runner-<pool>-<8 hex digits of a hash of its
  device id>`), so a second desktop adding the same pool to the same remote engine makes a runner
  of its own instead of colliding. A runner made before this is adopted at its next link: its
  container is renamed, and its home volume keeps its name. A runner another desktop made on the
  engine is never stopped, removed or adopted (#7192, part of #7180).
  A container made under a tagged name carries a `purlis.device` label, and the pool status
  counts the runners of another desktop by it. A purlis from before this refuses a
  `runners.json` that holds such a row, and an adoption's rename is not undone by a downgrade.
