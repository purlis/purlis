### Fixed

- **A chat that cleared its conversation still counts what the earlier one cost.** What a
  chat's harness reports it has cost is kept by conversation, so a task's dispatch record, the
  tokens on a tab's tasks and a session's token figure add every conversation the chat had,
  where before a `/clear` started the count again (#1457).
