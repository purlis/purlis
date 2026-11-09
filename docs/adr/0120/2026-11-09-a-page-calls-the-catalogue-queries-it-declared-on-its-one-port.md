# A page calls the catalogue queries it declared, on its one port

**2026-11-09, #5969.** Amends §4 and §7 of ADR 0120 with what building a page's catalogue
queries decided.

- **The queries are declared in the same block as the data files**, `"queries": {
  "purlis.todo_list@1": ["done"], "purlis.change_status@1": [] }`: each `<id>@<N>` with the
  parameters the page passes it. An id the catalogue does not have (a newer one's `replaces`
  counts), an `N` this build never shipped, or a parameter the query does not take makes the
  block declare nothing, as a bad data name does. `crates/purlis-core/src/artifact/calls.rs`.
- **A call is admitted only as declared**: the same `<id>@<N>`, written the same, and only
  declared parameters; otherwise `not-approved`. A parameter's value out of its closed type is
  `refused`. Each call is gated again when it is asked, as a data read is (the pin and this
  machine's approval read again).
- **It travels on the same port**, `{ id, query, params }` posted on `purlis.port`, carried as
  `/.purlis/port/<load>/query/<id>?<{query, params}>`, at most 8 KiB. No second channel.
- **Answers are data in a shape pinned per query** in `tests/fixtures/queries/v1.json`
  (`results`), read from the same stores the MCP tool reads; the MCP tool's text is unchanged.
  A page of memories carries its next page's `cursor` in the envelope.
- **Widening parameters are the catalogue's to name** (`Query::widens`): today
  `purlis.todo_list`'s `done`, which reads the done todos as well as the open ones. The bar
  lists each declared query and every widening parameter the page declared.
- **Plane scope is a grant of its own** (§4): an approved page reads its plane-scoped queries
  only once the person presses Allow project-wide reads in its bar, which records `plane: true`
  on the approval's entry (`docs/plane-format.md`). A new approval drops it. No core query is
  plane-scoped yet.
- **A deprecated or replaced id keeps answering**, and the bar warns of it in purlis's words.
- **`queries` is the MCP twin of `purlis queries`**, a tool that only reads the binary's own
  catalogue, pre-allowed with the other reads.
