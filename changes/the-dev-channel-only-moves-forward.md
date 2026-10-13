### Fixed

- **The dev channel no longer loses a build to a run that publishes nothing, or goes back to an
  older commit.** A dev build that was waiting for the one in progress could lose its place to
  a run started by a failed or cancelled check, which then published nothing, so its commit
  stayed unpublished until the next merge. And a build of an older commit could be published
  after a newer one, under a higher number, so machines on the dev channel took it. Now only
  a build that will publish waits, and a build publishes only when its commit comes after the
  one the channel serves. A build in progress is never cancelled. A waiting build can still be
  replaced by another commit's, and then the next green commit's build publishes it
  (`docs/updating.md`, "When a merge reaches the dev channel").
