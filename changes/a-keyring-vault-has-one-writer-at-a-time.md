### Fixed

- **Two writes to one keyring vault never lose each other's secret.** A `secret set`, a
  `secret delete` and the move of an item under purlis's access rule now take the vault's own
  lock, so a write that lands while another is in progress, or while an item is being moved,
  waits for it instead of being overwritten with an older value. The wait is bounded: a writer
  still held up after ten seconds is refused and writes nothing (#1180).
