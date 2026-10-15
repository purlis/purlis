### Added

- **A workspace's repo left off its default branch, or on a moved remote, is a Notice with a
  Fix.** The Inbox shows one Notice per repo: a repo itself on another branch than its default
  gets *Switch to `<default>`*, which switches it only when its tree holds no change, every
  commit of the branch is on the remote or another branch and no chat in it is mid-turn, and
  otherwise says what blocks it; a repo the forge said moved, on a fetch purlis ran, gets *Use
  the new address*, which points `origin` at the address the Notice shows (#1985).
