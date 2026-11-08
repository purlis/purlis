### Changed

- **A wide workspace's Explorer no longer reads every repo's status to count its changes.** The
  closed repo rows' change counts are one ask of the core, which keeps each repo's count until
  something in that repo moves, so a workspace of twenty repos no longer reads twenty statuses on
  every refresh (#3489).
