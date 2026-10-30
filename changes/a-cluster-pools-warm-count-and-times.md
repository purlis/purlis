### Added

- **A cluster pool keeps its own warm count, and names its times.** A `Pool` takes
  `warmReplicas` (0 unless set, 20 at most). The cluster operator keeps that many runners warm
  for each person who has a fresh claim on the pool, in their own namespace and counted in their
  quota, and brings a warm pool scaled by hand back to that count. A `Pool` also takes
  `idleAfter` (30m), `deleteSuspendedAfter` (720h), `noticeBeforeDelete` (168h) and `maxAge`
  (336h). The cluster refuses any other spelling and a notice that comes after the deletion. The
  operator does not act on the four times yet (#3737).

### Changed

- **A warm pool scaled with `kubectl scale` no longer keeps that count.** Set the Pool's
  `warmReplicas` instead (#3737).
