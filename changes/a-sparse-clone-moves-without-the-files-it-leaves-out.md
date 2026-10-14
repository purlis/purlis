### Fixed

- **A chat can move a sparse clone that leaves the editor's and agents' files out.** The guard
  on `git checkout`, `reset --hard`, `merge`, `rebase` and `pull` in a clone or a branch folder
  counts only the files git will write: a file the sparse checkout leaves out is not one
  (qw387).
