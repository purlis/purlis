### Changed

- **A runner holds a profile once as it keeps it, and a pool says how much its memory holds.** A
  runner kept up to three copies of what a profile sent while it kept it, and now keeps one. On a
  Docker pool, *Most a profile may send: size* is at most half the runner's Memory: the pool's page
  says so and refuses a larger value as it is typed, and a start over it says what Memory to set
  (#4396).
