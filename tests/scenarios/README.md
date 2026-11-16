# The sandbox scenario cells

One format for a cell's steps and expectations, shared by the Rust scenario suite and the
plane's live checker (`matrix-check.py`), so live and deterministic runs judge by the same
expectations (T1.1-05, #6852; M74.1's spec principles 2 to 4).

| Path | What it is |
|---|---|
| `cells/*.json` | Batches: cells run in order in one chat. Checked by `schema/cells.schema.json`. |
| `pins/<dialect>.json` | A dialect's defaults (ticket 3), in a `per_dialect` entry's shape. Checked by the same schema's `$defs/pins_file`. |
| `schema/leg-report.schema.json` | One record per entry a leg ran (ticket 6 writes them, ticket 8 judges them). |
| `src/` | The reader (`model`), the validator (`validate`), the verdict under marks (`judge`), and the production-timing set (`plan`). |
| `src/net/` | The suite's network on this machine (T1.1-02): a test CA new each run and the CA bundle its clients trust, local HTTP and HTTPS origins (plain, slow, redirect, their own 403) that record every request, and a direct-connection target off the loopback. |
| `tests/resolver_seam.rs` | purlis-core's resolver seam (`sandbox::names`, a test build's alone) with those origins, through a chat proxy and a browser's reach proxy. It binds local ports, so the suite's legs run it on Linux and macOS. |

`cargo test -p purlis-scenarios` runs every cells file through its schema and the validator.

## A cell, briefly

- `id` is the matrix row. `dialects` defaults to all three.
- A step's `require` and `forbid` name event kinds purlis-core writes. A step that forbids
  something also requires an event proving its call ran.
- Where a dialect lacks an event the cell names (Codex has no `PostToolUseFailure`), the cell
  says what that dialect shows: `per_dialect.<dialect>.shows` for every step, or
  `per_dialect.<dialect>.steps.<n>` for one step. The pins files give the defaults.
- `expected_fail` is a list, one mark per failing assertion and issue: `{issue, step,
  assertion, observed}`, with optional `leg`, `timings`, `dialect` and `os` scopes, or
  `{issue, check, refusal}` for a global check. An assertion is `require[i]`, `forbid[i]`,
  `window[i]`, `transcript[i]`, `outcome`, `calls` or `repeat_of`. A failure no mark pins is
  red, and so is a mark that pins nothing.
- Each mark's `note` cites the observation it pins from its run record. A mark scoped to
  timings the cell never runs at is refused.
- `template` names the `[policy]` template (Q308) a cell's chats start under: `open`, `watch`,
  `team` or `strict`. Every cell runs under `strict`, today's purlis, unless it names another
  (T1.1-06's scenario host; `every_cell_runs_under_strict_unless_it_names_a_template`).
- `"escape": true` marks an escape cell, which carries no `expected_fail`: a failing escape is
  a public `area:security`, `priority:p0` issue, fixed first (Q301). An escape's leg-report
  record is never `expected_fail`.
- `wait`, `within` and a press's `delay` are in Timings units (`hold+1`, `2*group`, `500ms`).
  A cell is production-timing when one of them names a member of a `production_only` pair,
  when a call carries a `timeout`, or when it restarts with `host_killed`. Only a hand
  `production_only` pair and `host_killed` skip scaled runs. `production_nightly_only` keeps a
  cell to the nightly production run.
