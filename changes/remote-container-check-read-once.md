### Changed

- **A link to a runner on another machine checks its container faster.** The check still runs
  whole before every link, and it refuses exactly what it refused before. It now reads the
  Docker daemon, the container and its home volume once each, all three at once. On a link
  that has just opened its connection, it takes the daemon's settings from the engine check
  that runs before it. A pool whose connection is slow to open no longer holds up the other
  pools' connections or starts.
