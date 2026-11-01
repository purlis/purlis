### Fixed

- **A push through purlis sends its Git LFS objects first.** A repo save, `purlis worktree push`,
  `purlis change push` and a project's first publish upload each object the commits going out
  need before the push leaves, to the push's own server with its one credential. Where that
  cannot be done, the push does not happen and purlis says what to do, so a forge is never left
  with a pointer nobody can download (#4399).
