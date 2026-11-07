### Fixed

- **A move that does not finish no longer hides a change.** Where moving a chat's work did not
  finish and purlis put the clone's index back, a file changed at the same size in the moment the
  index was last written could read as unchanged on Linux, missing from status and diff. The index
  put back now keeps the time it had (#5548).
