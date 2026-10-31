### Added

- **A pool's own time before its runner suspends itself.** Add a pool… has a
  **Suspends after** box: how long the pool's runner is left without a link from this machine
  before it stops itself, keeping its home, such as `12h` or `90m`, from a minute to 30 days.
  Empty is a day, as before. Each pool's row in Settings › This machine › Pools says its time,
  and `runners.json` keeps it as the pool's `suspendAfter` (#3471).
