### Changed

- **A chat's terminal is its own.** A sandboxed chat uses the terminal its session opened and any
  terminal it makes itself; a session's terminal opens for its owner alone, so `write` and `wall`
  from another account do not reach it.
