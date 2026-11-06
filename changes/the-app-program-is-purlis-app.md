### Changed

- **The app's program is now `purlis-app`, not `charter-app`.** On Linux the `.deb` installs
  `/usr/bin/purlis-app`, and its menu entry starts it under that name. A script or a shortcut of
  your own that runs `charter-app` needs the new name. No `charter-app` command is kept. The
  `purlis` and `charter` commands are unchanged. On macOS the program inside `purlis.app` is
  renamed and an update replaces the whole app, so nothing changes for you.
