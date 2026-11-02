### Changed

- **A chat's terminal takes its output once per frame.** A harness streaming a long message
  reaches the terminal as one write per frame instead of one per chunk, so the window does less
  work while it streams (#4732).
