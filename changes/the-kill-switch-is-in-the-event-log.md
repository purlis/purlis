### Added

- **The kill switch is in the event log.** As the app starts, each stop, re-arm and tamper in
  the kill switch's journal that the host's event log does not hold yet becomes a
  `killswitch.stop`, `killswitch.rearm` or `killswitch.tamper` event there, saying who wrote the
  line: the window's as a person's, the app's as purlis's own, and `purlis stop --all`'s as
  unattributed. While the app runs, each line is recorded as the switch writes it or its watch
  reads it. A line read again, or at the next start, is not recorded twice (#1375).

### Fixed

- **A tamper with the kill switch is journaled even when its marker cannot be put back.** The
  app used to journal a tamper only after restoring the stop marker; the journal line, which
  alone keeps the stop, is now written either way (#1375).
