### Added

- **A Docker pool's runners have limits and a read-only root.** Each runner container is
  held to its pool's CPU, memory, process and `/tmp` limits (by default two CPUs, 4 GiB,
  1024 processes and 1 GiB). Its root file system is read-only: its home volume and a `/tmp`
  that runs no program are the only places it can write. purlis checks both before every
  link (#2506).
