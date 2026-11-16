### Changed

- **A runner's chats ended by a shutdown, a stop or `purlis serve --remove` are kept as stopped,
  not crashed.** `purlis serve` now hears `SIGTERM`, which systemd and launchd send it, and
  `SIGINT` (Ctrl-C at a terminal) as a quit. It ends each chat as quitting does, waits for each
  chat's end to be written, leaves nothing for the next host to settle, and exits cleanly. A
  chat whose end is not written within five seconds is left for the next host to settle as a
  crash. The systemd unit now stops the host first (`KillMode=mixed`) and the LaunchAgent gives it
  30 seconds, so it has time to end its chats before anything is killed. A unit installed before
  this keeps the old way of stopping, which signals every chat at once, until you run `purlis
  serve --install` again on the runner. A host killed outright, by `SIGKILL` or a power loss, is
  still settled as a crash by the next one.
