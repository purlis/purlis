### Added

- **A chat's Chat log lists its blocks, each as a sentence.** A row says who blocked what
  ("purlis blocked a reset that would destroy commits"), never a guard's id. A tag shows
  where the sandbox, the harness's own check, an unverified report or Can't tell is the
  source; purlis's own consents carry none. Open a row to see its source and, while purlis
  runs, the reason the harness's check gave, exactly as it wrote it: the first it gave for
  that call, up to 16 KiB, and only under a block the harness's check made. The reason is kept
  in memory only and never written to the log (R-183, R-185 to R-188, R-340, #6420).

### Changed

- **A stop is called Blocked, never Refused.** A chat's Network tab, the Inbox's updates, the
  skill tables, a vault ask's notification, a commit a guard stopped (Commit blocked) and the
  dispatches refused while you were away say Blocked. A theme purlis
  will not apply says Not applied (R-170, #6420).
