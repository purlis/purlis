# Amended 2026-11-08: a chat's tools list and reopen a done todo (#3416)

The record's consequence *"A chat's MCP tools cannot list or reopen a done todo yet"* no longer
holds. The terminal, the window and a chat's tools now reach the same done todos:

- **`todo_list` with `done: true` lists the done ones**, each by its slug and title, as
  `purlis ws todo --done` lists them. It reads `todos/archive/` and makes nothing, so it stays a
  read, pre-allowed with the others (ADR 0064). Left out, it lists the open todos as before.
  An optional argument rather than a tool of its own, so the list of pre-allowed reads did not
  change.
- **`todo_reopen` takes a done todo's slug, or the end of one as `todo_done` does, and opens it
  again**, as `purlis ws todo reopen` does: the file moves back to `todos/` under its name, its
  index line comes back, and nothing is journalled. An open todo that already has the name is
  never replaced; the done one stays done.
- **`todo_reopen` runs without the harness asking**, beside `todo_done`: ADR 0064, amended
  2026-11-08.
