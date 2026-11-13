### Added

- **Each chat has a Chat log tab**, opened from the chat's list (Open ▸ Chat log) or the
  palette ("Chat log of …"): the chat's own timeline, apart from its transcript and beside its
  Activity, which is unchanged. Its first row says in one line what protected the chat as it
  started: the sandbox level, who set it, the `charter.toml` commit and the answers in force.
  Its last row is the chat's end and its result: done, failed with its exit code, stopped, or
  started again because the project's settings changed. A chat on a runner says which runner
  keeps its log. It is read when you open it and never counts, marks or announces anything.
  The event log now records a chat's own end, with its result, as its program ends.

### Changed

- **A runner's "While you were away" section now counts the chats that finished there**, as
  ADR 0078 §7 meant it to: the runner's event log now records each chat's own end, so the
  section has them to count. A chat started again because its project's settings changed did
  not finish, and is not counted.
