### Fixed

- **A CI-file push you approve no longer undoes the default branch.** When a chat's branch was
  cut from an older point, the one commit purlis made through the forge for it carried the
  branch's whole tree onto the newest default branch, and so reverted everything the default
  branch had gained since. Now such a push is not offered, nor performed at your press: you and
  the chat are told the branch is behind and to run `purlis worktree update --merge` and push
  again. A branch that contains the newest default branch pushes exactly its own changes (#7690).
