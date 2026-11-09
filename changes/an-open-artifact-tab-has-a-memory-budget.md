### Added

- **An open artifact tab has a memory budget.** ADR 0086 gains row M8: what an open artifact tab
  adds to purlis's memory, at most 192 MB to begin with, measured on every merge by the scenario
  tests on macOS and Linux, which fail past it (#5958).
