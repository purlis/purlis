### Fixed

- **Chats put back together at launch all start on macOS.** A Claude Code chat's settings there
  are long enough to go through a file of their own, and two chats writing theirs at the same
  moment, in a project none had written for yet or under one id, could leave one of them not
  started with "a chat's settings could not be written to its file". The second write now finds
  the folder or the file the first made and starts.
