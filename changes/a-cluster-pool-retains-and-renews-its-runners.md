### Added

- **A cluster pool's `deleteSuspendedAfter`, `noticeBeforeDelete` and `maxAge` now act.** A kept
  Claim suspended for being idle for 30 days is deleted with its volume, after a seven-day notice on
  the Claim. A `stopped` Claim or Pool is never deleted. A runner past its 14 days starts on the
  pool's current image the next time it is idle; a chat is never ended for it (#3981, #3982).
- **A cluster Claim says when its runner is stuck**, and the operator removes what deleted Claims
  left behind (#3739).
