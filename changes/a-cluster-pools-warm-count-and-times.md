### Added

- **A cluster pool keeps its own warm count, and names its times.** A `Pool` takes
  `warmReplicas` (0 unless set): the cluster operator keeps that many runners warm for fresh
  claims in each person's namespace, and a warm pool scaled by hand is brought back to it. It
  also takes `idleAfter` (30m), `deleteSuspendedAfter` (720h), `noticeBeforeDelete` (168h) and
  `maxAge` (336h). The cluster refuses any other spelling and a notice that comes after the
  deletion. The operator does not act on the four times yet (#3737).

### Changed

- **A warm pool scaled with `kubectl scale` no longer keeps that count.** Set the Pool's
  `warmReplicas` instead (#3737).
