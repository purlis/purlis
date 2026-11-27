### Added

- **A file a chat on a cluster pool prints opens too.** On a chat that runs on a cluster pool's
  runner, or on its own fresh runner there, a path the chat prints links and opens read-only, read
  from inside that runner, as it already does on a Docker pool's runner. A file outside the chat's
  workspace on the runner asks first (#6949).
