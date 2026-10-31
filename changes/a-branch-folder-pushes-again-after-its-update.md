### Fixed

- **A branch folder pushes again after its update, with commits added since.** `purlis worktree
  update` notes the commit the remote's copy of the branch was at, and a plain `purlis worktree
  push` goes over exactly that commit, so a pull request keeps its branch and history instead
  of being rebuilt on a new one. A remote someone else moved, a branch rewritten any other way,
  or remote commits the folder never had still refuse the push, and say what to do
  (#3895).
