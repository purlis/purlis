### Fixed

- **A branch folder whose pull request closed with its head on origin is removed without
  `--force`.** `purlis worktree remove`, `worktree prune` and a task folder's removal let it go,
  and where that head is on no branch of origin the refusal says the pull request closed. A
  refusal now names every reason at once (commits, uncommitted changes and ignored files), and
  `.astro/`, `.vite/` and a `node_modules/` holding only Vite's cache no longer keep a folder
  (#4500).
