### Fixed

- **A merged task's folder that holds git state of its own is no longer taken away by itself.**
  A ref only that folder has (`refs/worktree/…`, a bisect or a rebase in progress), its own
  `config.worktree`, and a commit only its HEAD's history holds (made, then amended or reset
  away) go with the folder when git removes it, and nothing else counted them. Such a folder
  now stays, listed with Discard (#1472).
