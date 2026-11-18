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

A job's `timeout-minutes` is its budget: exceeding it fails the job. They start at 15 minutes for
the Linux pull request job and 60 for the macOS job, and are set from the runs named here.

| Job | Budget (min) | Measured |
|---|---|---|
| `sandbox-scenarios-linux` | 75 | to be measured |
| `sandbox-scenarios-macos` | 120 | to be measured |
| `macos-sandbox-tests` | 120 | to be measured |
| `linux-release-profile` | 150 | to be measured |

Nightly entries carry theirs in `.github/nightly-on-phase-1-proven.txt`.
