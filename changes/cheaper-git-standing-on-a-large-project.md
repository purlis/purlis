### Changed

- **A large project's saving status costs a fraction of the git work it did.** Where your git
  supports them, a save leaves git's untracked cache and, on macOS and Windows, git's own file
  system monitor in the project's index, so the title bar, the Saving tab, the alerts and
  auto-save read the project without opening every folder or checking every file. On macOS the
  project's one watch tells purlis when anything moved, so an idle window no longer reads git
  every ten seconds. With an older git, purlis reads the project as before.
