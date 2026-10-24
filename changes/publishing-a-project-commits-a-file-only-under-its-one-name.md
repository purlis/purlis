### Changed

- **Publishing a project commits a file only under the one name it has on disk, as a save
  does.** Publish refuses a file with a second name (a hard link), or one that is not the file
  that was staged, names each one, and makes no commit and no push. A new project's first
  commit and a `purlis doctor --fix` commit are held to the same rule.
