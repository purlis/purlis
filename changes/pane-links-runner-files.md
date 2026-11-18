### Added

- **A file a runner chat prints opens too.** On a chat that runs on a Docker pool's runner, a
  path such as `src/main.rs:42:7` links once the runner finds the file. ⌘-click (Ctrl-click off a
  Mac) opens it read-only in an editor tab, read from inside the runner: its title says which
  runner it is in. Only a text file of up to 2 MiB opens. A file outside the chat's workspace on
  the runner asks first, as one on your machine does. purlis reads the file itself and never runs
  a command built from the path (#6387).
