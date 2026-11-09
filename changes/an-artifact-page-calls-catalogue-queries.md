### Added

- **An approved artifact calls purlis's read-only queries.** Its page declares them beside its
  data files, as `"queries": { "purlis.todo_list@1": ["done"] }` in its `purlis` block, and asks
  on `purlis.port` with `{ id, query: "purlis.todo_list@1", params }`. Each answers data in a
  shape `purlis queries` lists. Its tab's bar lists the queries and every parameter that widens
  what they read; anything undeclared answers `not-approved`. A query that reads the whole
  project needs Allow project-wide reads, apart from approving the script. Chats get `queries`,
  the MCP twin of `purlis queries`.
