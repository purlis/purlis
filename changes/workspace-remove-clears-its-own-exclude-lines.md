### Fixed

- **`purlis workspace remove` clears the exclude lines it left in a repository kept
  elsewhere.** When a checkout in the workspace is a linked worktree of a repository outside
  the project, the removal now takes out of purlis's block in that repository's
  `info/exclude` only the lines no other checkout purlis wires there still needs, and the whole
  block when none does. Your own lines in that file are never touched. When purlis cannot
  account for every line in the block, it leaves the block whole and names it, as before
  (#998).
