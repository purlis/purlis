### Added

- **A chat can list and reopen done todos.** Its `todo_list` tool takes `done: true` to list the
  workspace's done todos, as `purlis ws todo --done` does, and the new `todo_reopen` tool opens
  one again by its slug, as `purlis ws todo reopen` does, writing nothing to memory (#3416).
