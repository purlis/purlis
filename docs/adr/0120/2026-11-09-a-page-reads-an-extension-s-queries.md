# A page reads an extension's queries

**2026-11-09, #5971.** Amends §7 and §9 of ADR 0120 with what building extension queries
decided, and closes the two items #5970 left open.

- **A page declares an extension's query as it declares the core's**, `"slack.unread@1":
["channel"]` in its data block, and the block is read without looking the extension up: the
  page is shared, and what answers it is each viewer's own. Its parameters' names are checked for
  form only; the viewer's extension checks them when it is asked.
- **Admitted as a core query is**, declared as written with only declared parameters, and gated
  again at each ask; then answered by `purlis_core::extension::queries::ask` (ADR 0053 as amended
  2026-11-09). A plane-scoped one needs the page's plane grant, as a core one does.
- **The bar lists it** with the scope and widening parameters the viewer's extension declares, or,
  where this machine has none in force, says the query needs that extension and that part of the
  page is empty. The page is answered `needs-extension` and renders on.
- **`fresh_seconds` re-asks while subscribed** (#5970's open item): a feed of an extension query
  is asked again once its freshness has passed since its last run started, while someone
  subscribes, at most once per second as every feed is. The plane's changes do not move it.
- **The facts file's per-workspace key** (#5970's open item) is
  `workspaces.<ws>.queries.<name>`, with `queries.<name>` for a plane-scoped query.
- **Not yet**: a push when an extension's facts file changes. The plane watch does not watch an
  extension's state directory, so a page sees a facts-file answer at the next re-ask. The
  catalogue fixture (`tests/fixtures/queries/v1.json`) and `purlis queries` list the core's
  queries only; an extension's are in its manifest and the approval prompt.
