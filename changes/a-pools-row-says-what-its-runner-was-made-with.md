### Fixed

- **Settings › This machine › Pools says how each runner was really made to suspend.** A
  pool's row said "Suspends after …" from the pool's `suspendAfter`, even for a runner made
  before purlis suspended runners, which never suspends itself. The row now reads the runner's
  container: such a runner says "Never suspends: its runner was made before purlis suspended
  runners", and one made with another time than the pool's says the time it was made with
  (#3717).
