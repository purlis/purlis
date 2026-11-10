# A page subscribes to a query it declared, on its one port

**2026-11-09, #5970.** Amends §7 of ADR 0122 ("Live") with what building a page's
subscriptions decided.

- **A subscription rides the same port as a call**: `{ id, subscribe: "<id>@<N>", params }`
  posted on `purlis.port`, carried as `/.purlis/port/<load>/subscribe/<id>?<{query, params}>`;
  `{ id, unsubscribe: true }` as `/.purlis/port/<load>/unsubscribe/<id>`. Each answer comes back
  in the catalogue's envelope with the subscription's `id`, the first at once. A page that
  subscribes again with an `id` it used holds only the newer subscription.
- **It is admitted as a call is**: the pin and this machine's approval read again, only as
  declared, and a plane-scoped query only with its grant. A refusal is answered once, and
  nothing is subscribed.
- **"Core events" are the plane watch's change kinds** (`purlis_core::planechange`, #5955):
  each core query names the kinds its answer is read from (`purlis.todo_list` a workspace's
  todos, `purlis.memory_search` its memory, the session-record queries its sessions,
  `purlis.change_status` its own files, where `changes/` lies), in its own workspace.
  A burst the watch could not place moves every subscription. A query that names no kinds is
  moved by any change in its scope. `crates/purlis-core/src/artifact/live.rs`.
- **Coalesced per feed**: one feed per plane, workspace, `<id>@<N>` and parameters (none and
  `null` are one), whichever pages hold it. A page that joins a feed already answered is handed
  that answer without a run.
- **The rate limit is one run started per feed per second** (`live::MIN_INTERVAL`), with at
  most one in flight. A change heard sooner, or during a run, asks again once when the run is
  done and the second has passed, so a burst is one run and no page is left on an answer
  older than the last change.
- **Each push is gated again for each page**: its load still the open one, and its pin and
  approval still running its script. Otherwise the subscription is dropped, with
  `not-approved` when the approval went. A reload revokes a page's subscriptions with its port,
  as its tab closing and its window going do.
- **Nothing is kept**: subscriptions live in the app's memory for the run, so there is no new
  store and no tier.
- **Not yet**: an extension's query and its `fresh_seconds` (no extension contributes a query
  yet), and the facts file's per-workspace key.
