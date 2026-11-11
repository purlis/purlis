### Fixed

- **A full Docker disk is said plainly.** Docker keeps its containers on a disk of its own, apart
  from your computer's, and when that disk filled up, adding a Docker pool failed with only "the
  container is not running". Now purlis reads how much room Docker has before it makes the
  pool's runner, and refuses with less than 1 GB free, saying "Docker's disk is full" and how to
  make room: `docker system df`, `docker system prune`, or a bigger disk in Docker Desktop's
  settings. A runner that stops because the disk is full says the same, in the setup guide and
  on its pool in Settings › Pools.
