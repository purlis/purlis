### Added

- **`purlis queries` lists every query a page, a chat or a script can read through.** Each is a
  read-only tool of purlis's one tool catalogue, named `purlis.<name>` (`purlis.todo_list`), with
  its scope, the release it answers since, what it takes and the shape of its answer;
  `--json` prints the catalogue as JSON. Chats' MCP tools are unchanged.
