### Fixed

- **A release ships even when its runner image fails.** The desktop is published without a
  runner image; every pool stays on the runner it has, and the Pools group says the pool is not
  following (#2880).
