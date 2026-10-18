### Added

- **A Docker pool's runners have limits and a read-only root.** Each runner container is
  held to its pool's CPU, memory, process and `/tmp` limits (by default two CPUs, 4 GiB,
  1024 processes and 1 GiB). Its root file system is read-only, so its home volume is the only
  place it can write that may hold a program: `/tmp` and Docker's `/dev/shm` are
  memory-backed and run no program. purlis checks both before every link (#2506).
