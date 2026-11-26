# Amendment (2026-11-26): the macOS outcome bars run in a workflow of their own

The steward's ruling of 2026-11-26. `ci.yml`'s `outcome-bars` job, the Skill studio's outcome bars
on macOS, moves to `.github/workflows/outcome-bars.yml`, with the same triggers (main's push and the
nightly), job id, name, matrix and `continue-on-error`.

- **Why.** It was the last job of every main `ci` run to end: 36 to 72 minutes on main's push runs
  of 2026-11-25/26, while every other job ended within about 29. `release.yml`'s gate waits for the
  commit's whole `ci` run before a dev build signs, so a job that decides nothing held every dev
  build for 10 to 40 minutes. The release gate is unchanged. With the job gone, `ci` ends when its
  gating jobs do.
- **Why not parts.** The build took 30 to 65 of those minutes, and the three harnesses' runs 3.5 to
  6 together. Parts that each build, as the scenario parts do, would have saved about 3 minutes
  for two more Macs. The build is slow because purlis-core (1,578 s against Linux's 303) and the
  app (1,700 s against 751) compile on a 7.5 GB runner. That is a matter for the build, not for
  this rule.
- **Its Linux leg stays in `ci.yml`** (`outcome-bars-linux`, carried by the required check
  `outcome bars (ubuntu-24.04)`): it runs on pull requests and in the merge queue, and it ends
  inside the other jobs' time.
- **The steps are a copy** of `ci.yml`'s scenario steps, because an anchor does not reach another
  file. The standard ways to share them do not fit: a composite action's steps take no
  `timeout-minutes`, which the build (110) and the bars' probes (1) rely on, and a reusable workflow
  called from `ci.yml`'s scenario jobs would rename every check they post (`<caller> / <job>`),
  required ones among them. `tools/outcome-bars-workflow.test.mjs` fails when a copied step differs from its original,
  comments aside, or when `ci.yml` gains a step the macOS outcome leg would run and the copy lacks.
- **It stays watched.** Its budget is in `ci/job-budgets.toml` with `workflow = "outcome-bars"`
  (`JOB_WORKFLOWS` in `tools/job-budgets.mjs`). `ci-health` reads its runs for that budget, for
  the job durations, for the flaky tests and for "red on a green run".
- **sccache's server now lives as long as the job** (`SCCACHE_IDLE_TIMEOUT=0`, in the scenario
  steps). At the default of 600 s it stopped in all 8 of those runs, about ten minutes into
  purlis-core's compile, and every crate after that compiled without it.
- **Its runner is a repository variable**, `OUTCOME_BARS_RUNNER`, as `release.yml`'s dev build
  reads `MACOS_RELEASE_RUNNER` (#7641): unset, `macos-latest`. It is for the operator's 64 GB M1
  Max (the steward's ask of this day), in a runner group of its own (`purlis-bars`) that only this
  workflow on `main` may use. On that one Mac, main's pushes share one concurrency group, so runs
  never queue up behind each other. On a self-hosted runner it runs `release.yml`'s steps for a
  persistent Mac and never takes `sudo` (docs/updating.md, "The outcome bars on a self-hosted
  Mac"). The bars' limits were set on `macos-latest`. Re-setting them from the Mac's runs comes
  after it has some.
