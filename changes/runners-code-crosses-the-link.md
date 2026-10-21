### Added

- **Code crosses to a pool's runner and back as git bundles** (#2046). Over the link to a
  runner, the desktop sends a chat's branch and a snapshot of its uncommitted work, and takes
  the runner's branches back as `refs/remotes/<pool>/<branch>`. Your own branches, work tree
  and index are never touched. A runner keeps each project it holds for you in
  `<data>/projects/<project id>`. It takes only the branches and snapshots it was sent, runs no
  hook, checks every object as it arrives, and holds no forge credential. Session protocol
  1.34.
