# The sandbox scenarios' CI

How `.github/workflows/sandbox-scenarios.yml`, its actions and the nightly list run M74.1's
scenario suite (T1.1-09, T1.1-54). The decisions are Q285, Q296, Q300 and Q301 in the milestone's
decisions; this page says what runs when, and what each job is allowed to cost.

## What runs when

| Run | Linux | macOS (15 and 26) | Production timings | Release profile |
|---|---|---|---|---|
| Pull request into `phase-1-proven` or main | the suite | with `macos-scenarios` | with `production-timings` | when it changes a switch-list entry |
| `stress-x20` label | the changed batches ×20 under CPU stress | with `macos-scenarios` | | |
| Merge queue | the suite | the suite | the derived set, less M-NET-25 | |
| Nightly (main's scheduler, `.github/nightly-on-phase-1-proven.txt`) | the suite ×3, batches shuffled | the suite ×3, shuffled | the whole derived set, M-NET-25 at 15 minutes | every night |

- **×3 with shuffled batches.** A batch is a test's module path. Each run orders the batches from
  a seed (the run id and the run's number), never the cells inside one. A cell whose verdict
  differs between the three runs opens a `flaky-scenario` issue and the entry is red until it is
  fixed. An escape (`kind: escape`, a cell marked `escape`, or a step that expects a refusal)
  whose verdict differs opens a public issue labelled `area:security` and `priority:p0` instead:
  everything runs in public (Q301). `tools/scenario-legs.mjs flaky` does the comparison, and
  `--dry-run` prints the issue it would open.
- **`stress-x20`.** The batches the pull request changed (`tests/scenarios/cells/<B>.json`), or the
  whole suite when it changed none, twenty times under `stress-ng`, the verdicts compared the same
  way. M-NET-25 is never in it (Q300).
- **The production-timing run.** `plan::production_run` is what it takes: the nightly takes the
  whole derived set; the label's run and the merge queue's take it less the cells with
  `production_nightly_only` (M-NET-25), which they report skipped-for-timing and never as passed.
- **The run-result gate step** ends each job of `.github/actions/sandbox-scenarios`: it reads the
  job's reports and fails any covered entry, in a leg the job ran, with neither a pass nor a pinned
  `expected_fail`. A pull request's merge check is therefore the jobs its head ran, which its
  labels decided; the nightly and the merge queue run every leg's job. `live:` covers are judged by
  ticket 44.
- **Fixture and e2e covers** run on the OS their cover names (`tools/scenario-covers.mjs`), the
  e2e specs through the existing WebdriverIO setup, and write their records beside the legs'.
- **The existing macOS sandbox tests** are the job `macos-sandbox-tests` (and the nightly entry
  `existing-macos-sandbox-tests`); each target is a record of the leg report.
- **The Linux release-profile job** builds the bundle as `release.yml` does and runs one check for
  each Linux entry of `switches.json` (`tools/release-profile-checks.mjs`).

## Budgets

A job's `timeout-minutes` is its budget: exceeding it fails the job. They started at 15 minutes for
the Linux pull request job and 60 for the macOS job, and are set from what these runs measured.
The 15 minutes could not hold: a cold test build is 14 minutes on Linux and 28 to 30 on macOS
before a single scenario runs, so the budget is the build, the kit and the suite with a margin.

| Job | Budget (min) | Measured | Runs |
|---|---|---|---|
| `sandbox-scenarios-linux` | 45 | test build 14, kit 10, suite 2.5; the coverage gate's own build after it 12; 36 in all on a green run | [44267476720](https://github.com/purlis/purlis/actions/runs/44267476720), [44239993521](https://github.com/purlis/purlis/actions/runs/44239993521) |
| `sandbox-scenarios-macos` | 55 | macos-15: build 30, kit 4.5, suite 2.5 (38 in all); macos-26: build 28, kit 6, suite 4 (40) | [44267476720](https://github.com/purlis/purlis/actions/runs/44267476720) |
| `macos-sandbox-tests` | 100 | 77 | [44267476720](https://github.com/purlis/purlis/actions/runs/44267476720) |
| `linux-release-profile` | 60 | 37 on a cold cache (build 30 or so, the checks seconds) | [44267476720](https://github.com/purlis/purlis/actions/runs/44267476720) |

A `stress-x20` run is the same suite twenty times over under load, so it has a budget of its own
(Linux 150, macOS 180); a change that touches no batch a test is named for stresses the whole suite.

The nightly's entries run only from main's scheduler, after a merge, so their budgets are the
pull request runs' above with what the nightly adds: the suite ×3 (Linux 60, macOS 90), the
production-timing run (Linux 75, macOS 90: a build, the kit, and M-NET-25's 15 real minutes), and
the release profile (60). They are in `.github/nightly-on-phase-1-proven.txt`, and the first
nightly's run IDs replace this paragraph's derivation.

**Why the run-result gate is a program of its own.** A `cargo nextest run` after the suite finds the
workspace's test build stale and builds it again: 14 minutes on Linux, 21 on macOS in run
44267476720 (the matrix coverage gate pays the same 12 minutes on Linux). The gate
(`scenarios-gate`, built into the kit's target folder with `purlis`) needs no build.
