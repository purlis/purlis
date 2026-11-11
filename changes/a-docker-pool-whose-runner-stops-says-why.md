### Fixed

- **A pool whose runner stops as it starts now says why.** Before, adding a Docker pool whose
  runner's container stopped at once showed only that the container was not running, and the
  container was already gone, so there was nothing left to look at. Now purlis reads how it
  ended (its exit code, whether it ran out of memory, what Docker said) and the last line it
  wrote before removing it, says that in the setup guide with what to try next, and keeps the
  last 200 lines of its output in the app's log.
