### Fixed

- **A chat can move a sparse clone that leaves the editor's and agents' files out.** The guard
  on `git checkout`, `reset --hard`, `merge`, `rebase` and `pull` in a clone or a branch folder
  counts only the files git will write: a file the sparse checkout leaves out is not one
  (qw387).

- **`git rebase <upstream> 2>&1 | tail -5` in a clone is judged as `git rebase <upstream>`.** The
  shell's redirections are no longer read as git's words, so a move that names one commit and
  touches no denied file runs however its output is sent (qw387).
