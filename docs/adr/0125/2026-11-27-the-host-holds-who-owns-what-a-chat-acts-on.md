# Amendment 2026-11-27: the host holds who owns what a chat acts on

Amends §5 of [ADR 0125](../0125-blocks-are-off-by-default-and-opt-in-per-family.md) (one check
at each choke point) with the person's rulings **Q339.1, Q339.2, Q339.3, Q339.7 and Q339.8** of
2026-11-27 (#7748).

## `context.owner` is the host's, computed once

- A rule reads whose a thing is through `context.owner`: `"self"`, `"task"` (a chat down the
  actor's own dispatch line, at any depth), `"asker"` (one up it), `"other"` (any other chat, a
  sibling or cousin included) or `"none"` (no chat, or no record). It is never derived in Cedar
  from entity attributes.
- The host computes it **once, for the real actor**, before the decision is made
  (`policy::owner::of_request`, called outside `judged_then`), and every asker's evaluation up a
  dispatch chain (spec §6.5) reads that same value: an asker's evaluation never computes it again
  for the asker.
- **It is required**, `"none"` where the host holds no record (Q339.2), on `commit`, `push`,
  `update`, `discard`, `merge`, `reach`, `listen`, `browse` and `close`, on every forge operation,
  and on `read` and `write` (Q339.7), before the v1 schema is frozen (#7769).

## Its sources

- **Owner records in the sealed answer store**, an `owner` kind (Q339.3): which chat owns a forge
  branch (the one the forge route saw create it), which chat owns a branch folder (the first claim
  the app recorded), and which chat dispatched which. Only the app seals them, and the store's
  manifest and counter are anchored in the keyring. No owner is read from a file a chat can write:
  not the piece log, not the dispatch records, not `forge-created-branches.jsonl`.
- **The host's registry in memory** for a port's server and a browser: owned by the chat that
  opened them. A browser is `entity Browser`, and closing one is `action close` in the `browser`
  family (Q339.1).
- A record, once made, moves only by a finished task's hand-over to its asker or the person's
  press of "Let this chat push it".

## A lost record

- A record that is missing or fails its seal reads `"none"` for decisions, never `"self"`, and
  does not put the plane in the deny state (Q339.3).
- **It blocks every new owner record** (Q339.8). While the manifest lists an `owner` entry that
  does not load, `record_owner` and `record_dispatch` refuse: the key a lost record named can't be
  read, so no key is known to be free. A commit keeps the listing as it was, so the tamper leaves
  its trace, and the window shows a Notice. A manifest put back from before is the deny state, by
  the keyring's counter, as for every other kind.
- Rule authors write `context.owner != "self"`: a forbid on `== "other"` does not fire on
  `"none"`, which is what a lost or never-made record reads.

## `read` and `write`

They carry `owner` too, `"none"` today. The sandbox is compiled into a fixed profile, which can't
tell whose a path is, so a rule on `read` or `write` that names `owner` is refused when the
policy loads, as one naming `attended` or `harness_mode` is.
