### Added

- **A pool's limits at the command line** (#4284). `purlis pool limits <pool>` lists them
  (`--json` for a script), and `purlis pool limit <pool> <key> <value>` or `--reset` changes one,
  with the same names and checks as the window. In a chat it changes nothing: you're asked in the
  window, with the old and new value, and only you apply it.

### Fixed

- **A bad limit in the pool's file no longer stops every pool being listed** (#4381). That limit
  follows purlis's default and says why on the pool's page, and the rest keep working.
