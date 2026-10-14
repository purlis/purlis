### Fixed

- **Quitting waits for a project still being opened or closed.** A project closed or opened just
  before the quit is let go of with the rest, its last record and the window's tabs written, for
  up to ten seconds (#1949).
