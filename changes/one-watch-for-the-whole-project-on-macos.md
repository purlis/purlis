### Changed

- **A large project opens and idles without the file watcher's cost.** On macOS purlis watches
  the whole project with one stream, so setting up the watch no longer grows with the square of
  the folder count. An open window no longer re-reads every watched folder every five seconds.
  A full catch-up runs only when the system says it dropped events. On Linux, a folder the
  system will not watch, once it is out of watches, is checked on its own every few seconds,
  and the sidebar is no longer read from disk on every request (#5499).
