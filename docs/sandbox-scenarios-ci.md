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
  differs between the three runs is red until it is fixed, and, on the nightly alone, opens a
  `flaky-scenario` issue (the label is created when the repository has none): above five entries,
  one summary issue lists them instead. nextest rewrites its JUnit file on every call, so each
  batch's is kept under the run's folder and every run holds every test. An escape (`kind: escape`, a cell marked `escape`, or a step that expects a refusal)
  whose verdict differs opens a public issue labelled `area:security` and `priority:p0` instead:
  everything runs in public (Q301). `tools/scenario-legs.mjs flaky` does the comparison, and
  `--open` opens the issues and `--dry-run` prints the ones it would open. The issues are opened
  by a step of its own after the tests (`.github/actions/sandbox-scenarios`), the only step with a
  token that can write to issues, so no test process holds it; the jobs of `sandbox-scenarios.yml`
  have `issues: read`. A pull request's `stress-x20` run is a dry run: only the nightly opens a
  public issue. An issue that cannot be opened is an error, not a warning.
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

## The Claude Code leg (T1.1-10)

On macOS purlis does not wrap Claude Code: it hands the chat the `sandbox` object and the
`permissions.deny` rules of `sandbox::claude` in `--settings`, and Claude Code builds its own
seatbelt, with its Bash tool's network going through purlis's proxy. The fake harness cannot play
that, so `app/src-tauri/src/scenarios/claude_leg.rs` runs the real binary in the scenario host.
It is part of the suite on macOS (`scenarios::claude_leg::`), so it runs where the suite does: on
a pull request with `macos-scenarios`, in the merge queue and in the nightly. On Linux its tests
report `not_run`: purlis wraps Claude Code there (ticket 53's leg).

- **The binary** is the version `tests/fixtures/harness-versions/claude.txt` pins, installed by a
  step of `.github/actions/sandbox-scenarios` from the committed lockfile with no install scripts
  and named by `PURLIS_CLAUDE_CODE`. On macOS CI a leg without it fails; it never skips.
- **Started by the host's own chat start**, never by hand (`Scenario::starts_claude_code`).
- **A stand-in model** (T1.1-03's, on a loopback port the scenario's child binds) asks for each
  step's Bash call. No model is called and no credential is used. Claude Code's keep-list gains
  four variables, named in the test plane's `[chat_env] pass` (`claude_leg::ADDED`) and listed
  as substitutions in `switches.json`.
- **The transcript is the stand-in's request log**: what the real harness showed the model.
- **The canary** runs first: one write outside the project and one direct connect, both of which
  Claude Code's own sandbox must refuse. A command that gets through fails the leg.

## A nightly entry that fails or is cancelled

A job that exceeds its `timeout-minutes` reads as *cancelled*, not failed. The nightly's steps that
report an entry (`nightly-branch.yml`: what a red night leaves, and its issue) therefore run when the
entry fails or is cancelled (`failure() || cancelled()`), so a budget overrun opens the red night's
issue like any other red.

## Budgets

A job's `timeout-minutes` is its budget: exceeding it fails the job. They started at 15 minutes for
the Linux pull request job and 60 for the macOS job, and are set from what these runs measured.
The 15 minutes could not hold: a cold test build is 13 to 16 minutes on Linux and 28 to 55 on macOS
before a single scenario runs, so the budget is the build, the kit and the suite with a margin.

| Job | Budget (min) | Measured | Runs |
|---|---|---|---|
| `sandbox-scenarios-linux` | 45 | test build 13 to 16, kit 10 to 14.5, suite 2 to 3: 33 minutes to the coverage gate; the gate's own build after it was 12 until `issues.json` stopped being rewritten (below) | [44329073215](https://github.com/purlis/purlis/actions/runs/44329073215), [44301286922](https://github.com/purlis/purlis/actions/runs/44301286922), [44239993521](https://github.com/purlis/purlis/actions/runs/44239993521) |
| `sandbox-scenarios-macos` | 90 | macos-15: test build 55, kit 9, suite 3 (69 in all); macos-26: build 43, kit 12, suite 7 (63); on the merged base 60 and 61. A cold macOS build is 28 to 55 minutes; the worst of it with the kit and the suite after is 76 | [44301286922](https://github.com/purlis/purlis/actions/runs/44301286922), [44329073215](https://github.com/purlis/purlis/actions/runs/44329073215), [44267476720](https://github.com/purlis/purlis/actions/runs/44267476720) |
| `macos-sandbox-tests` | 110 | 77, 71 and 85 | [44267476720](https://github.com/purlis/purlis/actions/runs/44267476720), [44301286922](https://github.com/purlis/purlis/actions/runs/44301286922), [44329073215](https://github.com/purlis/purlis/actions/runs/44329073215) |
| `linux-release-profile` | 50 | 37, 40 and 32 (`purlis` and the extensions 12, the app 22, the checks seconds) | [44267476720](https://github.com/purlis/purlis/actions/runs/44267476720), [44281556559](https://github.com/purlis/purlis/actions/runs/44281556559), [44301286922](https://github.com/purlis/purlis/actions/runs/44301286922) |

**Watch item: thin headroom.** The release-profile job's worst run is 44 minutes of its 50, and
the Linux job's 37 of 45. A cold cache or a slower runner can overrun either; an overrun reads as
cancelled, and on the nightly opens the issue above. Re-measure these two first when a run comes
near its budget, and raise the budget in the workflow and this table together.

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
**The marks' issues refresh must not touch an unchanged file.** The coverage gate's own build after the
suite was 12 minutes more on Linux (44239993521, 44329073215): `node tools/scenario-coverage.mjs issues`
rewrote `tests/scenarios/issues.json` with the same issues and a new day, cargo takes any file of a path
package for its source, and `purlis-scenarios` and `purlis-app`'s tests built again. It writes only when the
issues differ now.

**The run-result gate is a program of its own** all the same (`scenarios-gate`, built into the kit's
target folder with `purlis`): it needs no build, whatever a later test does to the target folder.
