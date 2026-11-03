### Added

- **A vault key can be a linked entry**, which holds no value of its own and reads another vault
  entry's current value, so a rotation reaches every key linked to it. A link reads one entry that
  holds a value, never another link. `purlis vault verify` names a link whose entry is gone, and
  the vault view shows each link with the entry it reads and says how many links break before an
  entry they read is deleted (#5069).
