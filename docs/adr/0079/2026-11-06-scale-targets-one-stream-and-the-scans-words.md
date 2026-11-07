# Amended 2026-11-06: scale targets, one stream on macOS, and the scan's words

**Ruled by the person** on 2026-11-06 ("yes to all", "agree at all": the scale grilling's Q1 to
Q21, spec #5486), and built by #5498. Four of the rulings amend this record, and building it as
written settled four smaller questions it had left open.

## Ruled

1. **The budgets are the person's scale target.** §4's table is replaced by this one. Each row
   holds at 1,000 workspaces, 100 personas, 100,000 workspace memories and 100,000 persona
   memories (Q1, Q14). The nightly scale run (#5497) measures them on macOS and Linux against a
   generated project at that size. No pull request asserts a timing (ADR 0086, #4419).

   | What | Budget | From |
   |---|---|---|
   | A full rebuild of the project index | ≤ 40 s, in the background | Q14 |
   | A project search | under 100 ms | Q14 |
   | `recall`, and `recall --all-workspaces` | p95 ≤ 100 ms | Q1 |
   | The SessionStart hook | p95 ≤ 50 ms | Q1 |
   | macOS watcher setup | ≤ 1 s | Q14 |

   The two transcript rows (KN-32, D-0079c) stand as they were, for the machine index (item 3).
   The first scale run (#5497) measured the file scan at 152 ms (macOS) and 92.7 ms (Linux) for
   `recall`, and 121 s and 46 s for `recall --all-workspaces`.
2. **The watcher on macOS is one recursive FSEvents stream** over the project root, with paths
   filtered in purlis (Q13, Q15). §4's "still without recursion" and ADR 0068's amended bullet
   hold on Linux only, where `planewatch` keeps its non-recursive inotify watches with the memory
   and session folders among them, and a folder whose watch is refused falls back to a timed
   check of that folder alone (Q21). The stream itself is #5499's.
3. **The machine index over transcripts is out of scope** for the scale work (#5486). It stays as
   this record has it, for KN-31 and KN-32 to build.
4. **`recall` ranks by BM25 and keeps prefix matching** (Q16). Through the index it finds the
   same memories the file scan finds, for every reach; only their order changes.

## Settled in building it (#5498)

- **The index is handed the scan's words, not `unicode61`'s.** §9 named
  `unicode61 remove_diacritics 2`. That tokeniser finds `cafe` in `café` and `case` in
  `snake_case`, and the scan finds neither, so recall through the index would answer a
  different question than recall without it (§7). The writer splits each text as the scan
  matches it (runs of `\w` characters, lowercased, `_` kept), joins the words with spaces, and
  FTS5's `ascii` tokeniser with `_` as a token character splits them back unchanged. A prefix
  query `"term"*` then matches exactly where the scan's `\bterm\w*` does. The test that compares
  the two answers fails under `unicode61`.
- **A row keeps its title, after the scanner.** §6 says the index holds no text as written. The
  briefing (#5503) needs the newest titles and the oldest todos without opening a folder, and
  recall prints titles, so each row keeps its title with every span the shape scanner flags
  replaced by its kind, as its words are. The body is never kept. A workspace's summary row keeps
  the first line of its vision the same way.
- **The reach is in the FTS query.** Each row's FTS entry has a third column, `reach`, holding a
  word for its kind and one for its kind and owner (the owner in hex). A query names its reach in
  that column, so FTS5 ranks only rows the caller may see and stops at the limit, and the row
  table is filtered by kind and owner again. The column has weight 0 in BM25.
- **What recall still scans.** The ephemeral base is never indexed (§1), so an ask that names it
  is answered by the scan. So is every ask while there is no index this build can read.

## What changed where

| Where | Change |
|---|---|
| `crates/purlis-core/src/projectindex/` | The index: its writer, its read-only reader, its sources and reach, and the host's background keeper |
| `crates/purlis-core/src/recall.rs` | `recall` reads the index and falls back to `scan`, the file scan as it was |
| `app/src-tauri/src/hostsaving.rs` | The host keeps each project's index it saves, from the watch's paths and auto-save's moves of the tree |
| `docs/plane-format.md` | The index's two stores, written now |
| `docs/spec.md` | L5, K1 and G1 stated at the scale target |
