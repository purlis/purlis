# Amended 2026-11-07: the briefing reads the index

**Built by #5503**, from the person's rulings Q3 and Q17 of 2026-11-06 (spec #5486). The
SessionStart briefing reads the project index read-only, and walks no folder of the project for
what the index answers. Building it settled three questions the rulings left open.

## Settled in building it (#5503)

- **Three cases, not two.** With an index this build can read, the briefing reads it, even when
  no host keeps it (as `recall` does, §5). With no index while a writer holds `writer.lock`, the
  host is building the first one: the briefing leaves out the other workspaces, the memory
  counts and the open todos, says the index is being built, and scans nothing in their place.
  With no index and no writer, nobody will build one, so the briefing reads the files as it did
  before. That is the CLI's rule for `recall` with no host, and it keeps deleting the index safe
  (story 8 of #5486) and the recorded `sessionstart-*` answers (ADR 0046) unchanged.
- **The newest titles are still each store's `MEMORY.md`.** That is one file per store, not a
  walk. It keeps each line's hook, which the index does not hold. The counts, and what a store
  could not read, are the index's.
- **The host records the unshared memory in the index.** The keeper writes `meta.unshared`, the
  persona and workspace memory or ref files the project's git standing has uncommitted. It
  writes it when it starts, after each batch it is told, and after each auto-save look that did
  something. It reads the host's shared standing, so git runs no more often for it. The hook
  runs no `git status`. A working tree git cannot read is not recorded as such, because the
  standing does not say so: with an index, that case says nothing.

## What changed where

| Where | Change |
|---|---|
| `crates/purlis-core/src/briefing.rs` | Memory counts, oldest todos, other workspaces and the unshared line come from the index |
| `crates/purlis-core/src/projectindex/` | `Writer::record_standing`, `Reader::unshared`, `being_built`, and `Keeper::git_moved` |
| `app/src-tauri/src/hostsaving.rs` | The host tells the keeper after each look that did something |
| `docs/plane-format.md` | `meta.unshared`; the briefing is a reader of the index |
