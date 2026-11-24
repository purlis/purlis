### Fixed

- A sandboxed chat's plain `git fetch` and `git pull` no longer fail or hang on automatic `gc`: a chat's git is given `gc.auto=0` and `maintenance.auto=false`. The app's own git, such as the fetch behind `purlis worktree update`, still runs maintenance as before (#7531).
