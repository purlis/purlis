### Fixed

- **A persona's MCP server no longer fails at random as its chat starts.** A credentialed server
  that asked for its run before the app had finished recording the chat as started was refused,
  and Claude Code reported it as closed (`CONNECTION_CLOSED`) for the life of the chat; it
  happened most when several chats started together. The app now waits for a chat that is still
  starting (qw381).

### Added

- **A persona's MCP server that fails at chat start is tried once more.** When the second try
  fails too, the chat's tab says so with the server's own error and Retry, which restarts the
  chat on its conversation, and the chat is told why its tools are missing (qw381).
