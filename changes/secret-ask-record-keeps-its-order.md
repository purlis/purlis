### Fixed

- **A Secret ask's record keeps its order.** An ask raised before the event log opens is
  recorded as it opens, so its answer is never there without it. A Run grant's raise is in the
  chat's trace before its answer (#5509).
