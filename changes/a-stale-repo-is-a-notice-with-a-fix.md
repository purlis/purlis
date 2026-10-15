### Added

- **A workspace's repo left on a chat's branch, or on a moved remote, is a Notice with a Fix.**
  The Inbox shows one Notice per repo: a repo on a branch purlis cut for a chat or a task, not on
  its default branch, gets *Switch to `<default>`*, which switches it only when its tree holds no
  change and every commit of the branch is on the remote or another branch, and otherwise says
  what blocks it; a repo the forge said moved, on a fetch purlis ran, gets *Use the new
  address*, which points `origin` there (#1985).
