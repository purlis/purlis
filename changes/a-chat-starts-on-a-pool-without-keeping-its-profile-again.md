### Changed

- **A chat starts sooner on a pool whose runner already holds its profile.** purlis no longer
  links to the runner to keep the profile again at every start. It does so only when the
  profile or the pool's limits changed, the runner is new, or the runner restarted.
