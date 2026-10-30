### Changed

- **A chat refused a `git pull` at the project root no longer asks you for it.** The refusal
  names `purlis worktree update`, which the chat runs itself. You get a needs-you item only
  when that update is left for a reason no chat can settle: a file the chat's sandbox denies,
  a conflict, or a project not put back whole (#3877).
