### Added

- **An artifact's page stays up to date without asking again.** It posts `{ id, subscribe:
  "purlis.todo_list@1", params }` on `purlis.port` for a query it declared, and hears the answer
  with that `id` at once and again each time the project changes what the query reads, until
  `{ id, unsubscribe: true }` or the page loads again. Pages asking for the same query share one
  run, and a query runs at most once a second.
