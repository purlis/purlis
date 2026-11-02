### Fixed

- **A busy folder no longer stalls purlis.** A write that finds its folder locked waits up to ten
  seconds, then says the folder is busy and to try again, and writes nothing.
