### Fixed

- **The branch watch backs off when the platform keeps losing track, or no watcher can be
  made.** A build that overflows the file watch's queue no longer has every clone's tree watched
  again on each burst: losses in a row watch the trees again at most once in 2 seconds, and still
  stop vouching for the clones at once. A watcher the system cannot make is tried again after a
  minute, not on every read of a clone's save state (#874).
