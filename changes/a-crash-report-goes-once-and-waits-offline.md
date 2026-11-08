### Added

- **The same crash is reported once per version, and a report made offline waits.** A panic
  already sent from this machine in the same purlis version is counted, not sent again, and the
  count goes with the next report that leaves. A report made while purlis's report service cannot
  be reached waits on this machine (at most 50, for 14 days) and goes, oldest first, once it can;
  Settings › You › Reports lists it as "Waiting to send" (#5693).
