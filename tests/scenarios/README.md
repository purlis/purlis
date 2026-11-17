# The sandbox scenario cells

One format for a cell's steps and expectations, shared by the Rust scenario suite and the
plane's live checker (`matrix-check.py`), so live and deterministic runs judge by the same
expectations (T1.1-05, #6852; M74.1's spec principles 2 to 4).

| Path | What it is |
|---|---|
| `cells/*.json` | Batches: cells run in order in one chat. Checked by `schema/cells.schema.json`. |
| `pins/<dialect>.json` | A dialect's defaults (ticket 3), in a `per_dialect` entry's shape. Checked by the same schema's `$defs/pins_file`. |
| `rows.json` | The row table (ticket 43, from test-matrix §3.22): per matrix row × dialect × OS, whether its Log, Window and Chat state an expectation. Ticket 8's list takes it in; the validator's `RowTable` reads its shape. A dialect a row doesn't apply to has no entries. Draft until the person approves the matrix's §3.22. |
| `escapes.json` | The escape table (ticket 43): every escape × protection (Full, Basic, off) with what purlis owes, its positive control, dialects, OSes, and the real-binary (ticket 53) and live (tickets 37, 44) columns, for ticket 8's gate and tickets 20–22. Generated on the plane with the matrix's `escape-table.md`; draft until the person approves it. |
| `coverage.json` | The matrix coverage list (ticket 8): a cover for every row × dialect × OS of `rows.json`, every row §3.20 leaves to fixtures, each dialect's real-binary network and file probes, and every escape × protection of `escapes.json`, with the caps on each reason. Generated once by `tools/scenario-coverage.mjs` and owned here: a suite replaces a row's `pending: <ticket>` with its cover. |
| `issues.json` | The issues the cells' `expected_fail` marks name, and whether each is open: the gate reads it on a pull request; the nightly `coverage-issues` entry refreshes it from GitHub first (`node tools/scenario-coverage.mjs issues`). |
| `schema/leg-report.schema.json` | One record per entry a leg ran (ticket 6 writes them, ticket 8 judges them). |
| `src/` | The reader (`model`), the validator (`validate`), the verdict under marks (`judge`), the production-timing set (`plan`), and the coverage gate (`coverage`). |
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
- `"escape": true` marks an escape cell, which carries no `expected_fail`: a failing escape is
  a public `area:security`, `priority:p0` issue, fixed first (Q301). An escape's leg-report
  record is never `expected_fail`.
- `wait`, `within` and a press's `delay` are in Timings units (`hold+1`, `2*group`, `500ms`).
  A cell is production-timing when one of them names a member of a `production_only` pair,
  when a call carries a `timeout`, or when it restarts with `host_killed`. Only a hand
  `production_only` pair and `host_killed` skip scaled runs. `production_nightly_only` keeps a
  cell to the nightly production run.

## The coverage gate (ticket 8)

`cargo test -p purlis-scenarios --test coverage` holds `coverage.json` to the matrix:

- **A cover** is `scenario: cells/<file>#<cell>`, `fixture: <test file>::<test fn>`, `e2e: <spec>#<test
  title>`, `live: <live-pass cell>` or `pending: <ticket>`. It must name a cell or test that exists,
  of the entry's `kind`. Every `pending` is gone by M74.1's gate.
- **Kinds and reasons.** `carried` is the kind ticket 43's approved column gives the row; a row moved
  off `scenario` names the ruling in `moved` (a Q-id). A `live`, `fixture` or `e2e` cover carries a
  reason from its closed list (`coverage::LIVE_REASONS`, `FIXTURE_REASONS`, `E2E_REASONS`), and each
  reason has a cap: `seed` from ticket 43's column, `cap` above it only with an `approved` Q-id.
- **Which leg counts.** A Claude Code scenario cover on macOS counts only from ticket 10's leg; a row
  with `binary: real` or `both` also needs ticket 53's; an escape the table marks `real_binary` runs
  there too. Fixture and e2e covers run on the OS their entry names. `judge_runs` reads the legs'
  reports and fails any covered entry with neither a pass nor a pinned `expected_fail` there.
- **The ratchet.** A change adds a mark on an existing cell (per cell × dialect × OS × leg ×
  timings) only with an issue no mark named before; a new cell may carry any open issue's. No mark
  names a closed issue, and a pull request closes no issue a mark names. `sandbox-scenarios-linux`
  runs the ratchet against the change's base (`SCENARIOS_BASE`) and the issues it closes
  (`SCENARIOS_PR_CLOSES`).
- **At M74.1's gate** (`--ignored`), nothing is pending, and every mark is one the person approved
  by name in `approved_marks`.
- **At a re-carry** of the matrix, the orchestrator runs `node tools/scenario-coverage.mjs diff
  <test-matrix-phase-1.md>` and re-emits `rows.json`; the gate fails until the list matches both.
