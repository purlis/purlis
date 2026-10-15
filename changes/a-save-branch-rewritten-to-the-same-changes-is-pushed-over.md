### Fixed

- **A request-mode save no longer blocks for good after the plane's history is rewritten.** When
  the save branch on the remote holds only commits the plane already has under new shas (the same
  changes, rewritten with new dates), the save pushes over it instead of saying it "has commits
  this clone did not push there". A commit on it that the plane does not have still blocks the
  save.
