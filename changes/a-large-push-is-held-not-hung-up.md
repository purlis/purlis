### Fixed

- **A chat's push larger than 1 MiB is read, and held when it changes CI files.** Git checks with
  the forge before it sends a large push, and the route refused that check, so git printed "the
  remote end hung up unexpectedly" and "Everything up-to-date", and no Ask was raised. The check is
  now let through and the push is read like any other. A refused push whose refs cannot be read
  says why in git's own output. A chat that purlis restarted is told to push with
  `purlis worktree push` or start a new chat (#7878).
