### Changed

- **A large project opens without waiting for its watch on macOS.** The project's one FSEvents
  stream starts first, and the folders whose changes reach the window are listed after it, on the
  watch's own thread. On a project of 1,000 workspaces that list is about 5,000 folders, and the
  open waited 0.8 to 2.2 s for it on a busy machine. A change made in the meantime is held
  until the list is ready, so none is lost (#5672).
