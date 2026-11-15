### Fixed

- **The session host's log no longer grows without bound.** On macOS, a session host that keeps
  running in the background writes to `session-host.log` in purlis's log folder. Once that file
  passes 1 MB, purlis moves its contents to `session-host.log.1` and starts the log again, keeping
  the three most recent older files. If that ever fails, `purlis doctor` says how large the log
  has grown.
