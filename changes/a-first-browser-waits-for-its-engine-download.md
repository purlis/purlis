### Fixed

- **The first browser on a slow connection starts.** A browser's start waits for its engine's
  download, however long that takes within its own limit, where before the window gave up after
  30 seconds and said the session host did not answer in time.
