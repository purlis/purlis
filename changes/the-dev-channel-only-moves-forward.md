### Fixed

- **The dev channel no longer skips a green commit or goes back to an older one.** A dev build
  that was waiting for the one in progress could lose its place to a run that published
  nothing, so its commit stayed unpublished until the next merge. And a build of an older
  commit could be published after a newer one, under a higher number, so machines on the dev
  channel took it. Dev builds now queue behind the one in progress, only a build that will
  publish waits, and a build publishes only when its commit comes after the one the channel
  serves (`docs/updating.md`, "When a merge reaches the dev channel").
