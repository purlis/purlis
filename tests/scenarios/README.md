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
| `issues.json` | The issues the cells' `expected_fail` marks name, whether each is open, and when it was opened: a local cache. `sandbox-scenarios-linux` and the nightly `coverage-issues` entry refresh it from GitHub before the gate reads it (`node tools/scenario-coverage.mjs issues`), so a change's own edit of it decides nothing in CI. |
| `rulings.json` | The person's rulings of M74.1 (decisions.md's `[person]` entries), by exact id, each with its date and what it approves (`rows`, `caps`, `marks`): the only Q-ids that move an entry, raise a cap or approve a mark in `coverage.json`. The gate reads the base's copy, so a ruling is added in a pull request of its own, merged before the one that cites it. |
| `schema/leg-report.schema.json` | One record per entry a leg ran (ticket 6 writes them, ticket 8 judges them). |
| `src/` | The reader (`model`), the validator (`validate`), the verdict under marks (`judge`), the production-timing set (`plan`), the coverage gate (`coverage`), and the live-pass tools (`livepass`, `src/bin/live-pass.rs`). |
| `src/net/` | The suite's network on this machine (T1.1-02): a test CA new each run and the CA bundle its clients trust, local HTTP and HTTPS origins (plain, slow, redirect, their own 403) that record every request, a direct-connection target off the loopback, and the fake forge (`forge.rs`, T1.1-50): a GitHub- and GitLab-shaped API for a merge that records every request and the token it presented. Its API is documented in the module's own docs. |
| `tests/fake_forge.rs` | The fake forge's own tests: purlis's forge client merges on it, a requested ruleset refusal, GitLab's `PRIVATE-TOKEN`, HTTPS under the test CA, and every credential every request recorded (each `Authorization` value, `PRIVATE-TOKEN`, `JOB-TOKEN`, and `private_token` or `access_token` in the query or body), so a second token on one request is listed. Binds local ports, so it runs on each OS's leg. The cells where `gh` and `glab` reach it from a sandboxed chat are the app's (`app/src-tauri/src/scenarios/forge.rs`). |
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

## The runner (#7322)

`app/src-tauri/src/scenarios/cells.rs` plays every cell in `cells/` in the scenario host, one chat
per batch and dialect, and reports each through `report_cell`, so a cell that ran is a cell record
(kind, row, `refusal_expected`, events) and not only a JUnit row (#7229). What a cell means is
decided in `src/play.rs`, which has no chat and is tested on its own: a step's `require` and
`forbid` read against the events its chat wrote (`step_assertions`; the file's docs say what
`count`, `before`, `at`, `within` and `args` mean), and the verdict (`verdict`).

**An unknown word is malformed, never passed.** A cell with one is reported `fail` with every
reason, so the run-result gate goes red on it. The words are closed lists, one place each:

- `window[].query` is a word of `play::Window`, each one of the window's own queries:
  `ask.listed`, `notice.standing`, `blocked_log.lists`, `chat_log.block_row`,
  `needs_you.queued`, `merge_row.listed`, `settings.grant_listed`. `forbid: true` asks for it not
  to hold. Prose is not a word.
- `press` is a label in `press_of` (`cells.rs`): the ask's buttons by the window's own wording
  (`allow_said`, `KEEP_SAID`), `Merge`, `Don't merge`, `Stop chat`, `Ignore the ask`, and
  `Kill the harness process from the person's terminal`. A button's `target` is `ask` and its
  `when` is `ask.raised`.
- `restart` and a second `chat` are not played yet: a cell with either is malformed.
- A mark's `check` is one the suite runs: `refusal_black_hole`, `ask_black_hole` or
  `block_without_source`.

**Q301.** An escape cell is never `expected_fail`, and neither is a failed `outcome` of a step
with `refusal_expected`: the refused action happened, so the record is a `fail` and carries
`refusal_expected` for the comparison of runs to open the escape issue.

Claude on macOS is ticket 10's leg, so the fake leg plays no Claude cell there.

## The coverage gate (ticket 8)

`cargo test -p purlis-scenarios --test coverage` holds `coverage.json` to the matrix:

- **A cover** is `scenario: cells/<file>#<cell>`, `fixture: <test file>::<test fn>`, `e2e: <spec>#<test
  title>`, `live: <live-pass cell>` or `pending: <ticket>`. It must name a cell or test that exists,
  of the entry's `kind`. Every `pending` is gone by M74.1's gate.
- **What every list owes**, from the gate's code, never from the list: each row the matrix leaves
  to fixtures (`coverage::FIXTURE_ROWS`: §3.20, and §3.19's M-POL-01…03) and each dialect's 24
  real-binary probes, whatever the list's `version`.
- **Kinds and reasons.** `carried` is the kind ticket 43's approved column gives the row; a row moved
  off `scenario` names the ruling in `moved`. A `live`, `fixture` or `e2e` cover carries a reason
  from its closed list (`coverage::LIVE_REASONS`, `FIXTURE_REASONS`, `E2E_REASONS`), and each reason
  has a cap: `seed` from ticket 43's column, `cap` above it only with an `approved` ruling. A
  ruling is one of the person's in `rulings.json` (decisions.md's `[person]` entries of M74.1), by
  its exact id (a point, `Q321.8`, is listed as its own), and it approves only what it names: a
  row, escape or probe in `rows`, a cap (`fixture.race`) in `caps`, an issue's mark in `marks`.
- **Against the base.** Editing the list alone does not move what the matrix and the rulings fix:
  `sandbox-scenarios-linux` holds the list to its base's (`against_base`), and its rulings to the
  **base's** `rulings.json`, never the change's own. A cap whose seed and cap rise above what the
  base allowed, a row whose `carried` moves, whose `kind` moves off what it carries, or whose
  `binary` drops a leg (`both` to `fake`), a real cover (a row's, a fixture row's, an escape's or
  a probe's) going back to `pending`, a fixture row's test leaving an OS, and a new
  `approved_marks` entry each need a ruling of the base's that names it and that the base's entry
  did not already cite. A ruling is therefore added in a pull request of its own.
- **Which leg counts.** A Claude Code scenario cover on macOS counts only from ticket 10's leg; a row
  with `binary: real` or `both` also needs ticket 53's; an escape the table marks `real_binary` runs
  there too. Fixture and e2e covers run on the OS their entry names. `judge_runs` reads the legs'
  reports and fails any covered entry with neither a pass nor a pinned `expected_fail` there: a run
  that ran the scenario pass on an OS owes every scenario leg its covers require there, listed in
  its legs or not.
- **The ratchet.** A change adds a mark on an existing cell (per cell × dialect × OS × leg ×
  timings) only with a new issue: one no mark of the base names, opened after the change began
  (its merge base's commit time, `SCENARIOS_SINCE`, which the job reads from GitHub; each issue's
  `opened` comes from GitHub at check time); a new cell may carry any open issue's. No mark
  names a closed issue, and a pull request closes no issue a mark names. `sandbox-scenarios-linux`
  runs the ratchet against the change's base (`SCENARIOS_BASE`) and the issues it closes
  (`SCENARIOS_PR_CLOSES`).
- **At M74.1's gate** (`--ignored`), nothing is pending, and every mark is one the person approved
  by name in `approved_marks`.
- **At a re-carry** of the matrix, the orchestrator runs `node tools/scenario-coverage.mjs diff
  <test-matrix-phase-1.md>` and re-emits `rows.json`; the gate fails until the list matches both.
  `diff` compares each row's carried kind and binaries, and the fixture rows, and fails on a §3 row
  that §3.22 doesn't carry and the matrix doesn't leave to fixtures.

## The live pass (ticket 37)

`tools/live-pass` is `livepass`'s command line (`cargo run -p purlis-scenarios --bin live-pass`). It
is harness agnostic: a dialect is only the key to the hook words it is armed with.

- `brief --harness H --os OS` writes a **record-only** runner brief, from the files above: every
  cell `coverage.json` covers `live:` (a row's, or an escape's or its control's); one control per
  hook kind the harness is armed with; the escapes `escapes.json` marks `live: yes`, at each
  protection that owes something; and on macOS one check per entry of
  `app/src-tauri/src/scenarios/switches.json` whose macOS covers include `macos-live-check`
  (ticket 6's reviewed list, read when the brief is made, never copied), ticket 25's timed (g)
  cell, and the denial receiver's non-admin check when a macOS `M-FILE` row's `fixture:` cover
  is `the_receiver_reports_for_a_standard_non_admin_user`. A `live:` cover that names no live-only
  cell for the harness is an error, never a quiet omission. The runner uses an installed
  **release-profile** build of `phase-1-proven`'s head, with no `test-support`, in the harness's
  non-interactive mode.
- `report --cells FILE [--cells FILE]... --log FILE --harness H --os OS` judges a live log by the cells' own
  `expected_fail` marks (`judge`) and writes the suite's report format (`leg-report.schema.json`),
  leg `live`, one record per line. `--cells` **repeats**, one file each, and takes exactly one: an extra argument (`--cells a.json b.json`) or an option the command lacks (`--celss`) exits 2, as does a cell id given twice in the log or in two cells files.
  The log is `{"cells": {"<id>": {"failed": [...], "globals": [...], "events": [...]}}}`, as
  `tests/fixtures/chat-682-b0.json` (chat 682, B0, 2026-11-15): `failed` is a list of
  `[assertion, observed]` (step 1) or `[step, assertion, observed]`, `globals` a list of
  `{"check", "refusal"}`, and `events` (optional) the cell's event kinds in order. `failed` and
  `globals` are **required** and no other key is read: a missing or wrong-typed field, a cell id
  no given file has for the harness, or a malformed `events` is an error, never a pass. A cell
  the log leaves out is `not_run`.
- Only cells are judged by `report` and `diff`. The controls, escapes, release checks, held-ask
  timing and receiver check have no record they read: the brief says so, and a person reads
  each one's notes against its `Record:` line before tagging.
- `diff LIVE SUITE [--suite-leg L] [--suite-timings T]` checks both files against the
  leg-report schema (the left one must be leg `live`), keeps the suite records whose row,
  dialect, protection and OS the live report has (a record with no `timings` is `scaled`), and
  compares per cell (same kind, row, dialect, protection and OS): the verdicts, and the event
  sequences where both sides recorded `events` (an optional field of the leg-report schema).
  A live `not_run` is always listed, a cell on one side alone is listed, and it prints how many
  pairs had no event comparison and which had events on one side only. **Every live `fail` is
  listed (`FAIL` lines; an escape cell's says the refused action happened) and blocks exit 0
  whatever the suite says**: a cell that fails live and in the suite is two reds, not agreement.
  It exits 0 only when it compared at least one pair and nothing differs, 1 when something differs,
  a cell failed live or nothing was compared, and 2 when a file, an option or an extra argument is
  wrong. It does not check that the live report covers every cell the brief asked for: the cells
  files given to `report` decide that, and a cell they leave out is not in the report at all.
  The scenario host writes `events` (`report::ran`, `Scenario::report_cell`) into the record of
  every cell the runner plays (`every_cell_plays_and_is_reported`), so those pairs compare event
  sequences. A suite record with none (a
  skipped-for-timing cell, a fixture) compares verdicts only.
