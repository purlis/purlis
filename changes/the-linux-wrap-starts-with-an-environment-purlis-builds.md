### Changed

- On Linux, purlis's own sandbox wrap starts with an environment purlis builds, and only the
  program it runs is given the chat's variables. This holds for a chat's start and for a
  brokered `secret exec`.
