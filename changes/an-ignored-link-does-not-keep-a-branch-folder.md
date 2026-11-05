### Fixed

- **A linked install no longer keeps a chat from removing its finished branch folder.** A folder
  whose ignored `app/node_modules` is a symbolic link into another folder, as lanes share an
  install, is removed like one with its own: an ignored link goes with the folder, wherever it
  points, and what it points to is never touched (#4245).
