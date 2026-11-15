### Changed

- **A runner's chats ended by a shutdown, a stop or `purlis serve --remove` are kept as stopped,
  not crashed.** `purlis serve` now hears `SIGTERM`, which systemd and launchd send it, as a
  quit. It ends each chat as quitting does, leaves nothing for the next host to settle, and exits
  cleanly. The systemd unit stops the host first (`KillMode=mixed`) and the LaunchAgent gives it
  30 seconds, so it has time to end its chats before anything is killed. Run `purlis serve
  --install` again on a runner to take the new unit. A host killed outright, by `SIGKILL` or a
  power loss, is still settled as a crash by the next one.
