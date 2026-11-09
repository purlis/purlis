# Amended 2026-11-09: catalogue queries, and protocol 7 (ADR 0120 §9, #5971)

The vocabulary gains one word:

| Word      | What purlis does                                                                                                                                              | Its shape, under `contributes`                                                                                      | Change |
| --------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------- | ------ |
| `queries` | answers an artifact's page reading `<extension-id>.<name>`, from the facts file while its answer there is fresh, otherwise by asking the program (protocol 7) | `queries`: `name`, `description`, `scope` (`workspace`, `plane`), `fresh_seconds`, `parameters`, `result`, `widens` | #5971  |

**The shape.** At most 16 queries. `name` is one id segment, the query's id after
`<extension-id>.`. `parameters` is a JSON Schema object whose properties are of the closed types
a core query's are (string, boolean, integer), at most 8, with `required` naming only those;
none is a query that takes none. `result` is the JSON Schema of the answer's `data`, at most
16 KiB. `widens` names each parameter that widens what the query reads, with what it then reads,
which the page's approval lists. A query needs a program (`runs`). The approval prompt names
each query, its scope, how long an answer is good for, and what widens it.

**Protocol 7 holds its one request.** `{"charter": 7, "extension": "<id>", "query": {"name":
"<name>", "workspace": "<ws>" | null, "params": {…}}, "writes": []}`: the workspace is `null` for a
plane-scoped query, and `params` are already checked against what the query declares. It is
answered `{"charter": 7, "data": <any JSON>, "cursor"?: "<text>"}` or an `error`, with the normal
deadline, and waits its turn behind a question in flight, as an event does. It is handed no plane
path, so anything it changes in the plane is reported. `queries` needs protocol 7, so a manifest
naming an earlier `version` is refused.

**The facts file gains a key per workspace.** `workspaces.<ws>.queries.<name>` holds `{"data",
"at"}` for one workspace's query, and `queries.<name>` for a plane-scoped one. While that answer
is younger than the query's `fresh_seconds` (and no more than five minutes ahead), a query asked
with no parameters is answered from it and no program starts. The badges and repo columns read
neither key.

**What a page is answered.** An extension not installed here, not approved, turned off in the
project or workspace, or not declaring the query, answers `needs-extension`: each viewer's own
extensions answer a shared page. One changed since its approval answers `not-approved`; a
parameter it does not take, or of the wrong type, `refused`; no answer within the deadline,
`timeout`; any other failure, `failed`. **On Windows, where purlis starts no extension's program,
every extension query is `refused`** (ADR 0031), never `needs-extension`.

It is proven end to end by the extension probe as a second manifest on the same program,
`query-probe` (`crates/extension-probe/purlis-extension.queries.json`), in protocol 7: the probe's
own manifest speaks protocol 2, and moving it would ask every capability it proves in a protocol
it was not proven in (the reason the Check reporter has `check-probe`).
