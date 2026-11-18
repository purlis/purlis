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
The 15 minutes could not hold: a cold test build is 13 to 16 minutes on Linux and 28 to 55 on macOS
before a single scenario runs, so the budget is the build, the kit and the suite with a margin.

| Job | Budget (min) | Measured | Runs |
|---|---|---|---|
| `sandbox-scenarios-linux` | 40 | test build 13, kit 10, suite 2, the rest seconds: 27 minutes. Before the coverage gate's second build was ended (below) the same job took 36 | [44301286922](https://github.com/purlis/purlis/actions/runs/44301286922), [44281556559](https://github.com/purlis/purlis/actions/runs/44281556559), [44267476720](https://github.com/purlis/purlis/actions/runs/44267476720), [44239993521](https://github.com/purlis/purlis/actions/runs/44239993521) |
| `sandbox-scenarios-macos` | 90 | macos-15: test build 55, kit 9, suite 3 (69 in all); macos-26: build 43, kit 12, suite 7 (63). A cold macOS build is 28 to 55 minutes; the worst of it with the kit and the suite after is 76 | [44301286922](https://github.com/purlis/purlis/actions/runs/44301286922), [44267476720](https://github.com/purlis/purlis/actions/runs/44267476720), [44281556559](https://github.com/purlis/purlis/actions/runs/44281556559) |
| `macos-sandbox-tests` | 100 | 77 and 71 | [44267476720](https://github.com/purlis/purlis/actions/runs/44267476720), [44301286922](https://github.com/purlis/purlis/actions/runs/44301286922) |
| `linux-release-profile` | 50 | 37, 40 and 32 (`purlis` and the extensions 12, the app 22, the checks seconds) | [44267476720](https://github.com/purlis/purlis/actions/runs/44267476720), [44281556559](https://github.com/purlis/purlis/actions/runs/44281556559), [44301286922](https://github.com/purlis/purlis/actions/runs/44301286922) |

A `stress-x20` run is the same suite twenty times over under load, so it has a budget of its own
(Linux 150, macOS 180); a change that touches no batch a test is named for stresses the whole suite.

The nightly's entries run only from main's scheduler, after a merge, so their budgets are the
pull request runs' above with what the nightly adds: the suite ×3 (Linux 60, macOS 120: a build, the kit, three suites), the
production-timing run (Linux 75, macOS 120: a build, the kit, and M-NET-25's 15 real minutes), and
the release profile (50). They are in `.github/nightly-on-phase-1-proven.txt`, and the first
nightly's run IDs replace this paragraph's derivation.

**A cargo run after the suite must not rebuild the workspace.** Until run 44301286922 it did: 14
minutes on Linux, 21 on macOS (the matrix coverage gate paid 12 on Linux every time, and the
production-timing step 15). The cause, reproduced locally: the resolver seam's compile check
(`resolver_seam::the_resolver_seam_does_not_compile_without_test_support`) ran `cargo rustc` in the
workspace's target folder, and that left `ring` and every crate over it stale. It builds in
`target/seam-probe` now, so the nightly's repeats and the stress run's twenty runs reuse one build.
**The run-result gate is a program of its own** all the same (`scenarios-gate`, built into the kit's
target folder with `purlis`): it needs no build, whatever a later test does to the target folder.
