### Changed

- **A chat starts sooner on a pool whose runner already holds its profile.** purlis no longer
  links to the runner to keep the profile again at every start. It does so only when the
  profile or the pool's limits changed, the runner is new or restarted, or the runner finds
  anything a keep would put back or clear: a settings file changed, added, gone or with its permissions changed, or another
  setting, rule, hook or MCP server left there since it was kept. A runner older than
  this version keeps the profile at every start, as before. The first start on each runner after
  purlis restarts keeps the profile once.
