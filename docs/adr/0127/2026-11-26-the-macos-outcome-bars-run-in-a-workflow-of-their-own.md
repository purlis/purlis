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
  file. `tools/outcome-bars-workflow.test.mjs` fails when a copied step differs from its original,
  comments aside, or when `ci.yml` gains a step the macOS outcome leg would run and the copy lacks.
- **It stays watched.** Its budget is in `ci/job-budgets.toml` with `workflow = "outcome-bars"`
  (`JOB_WORKFLOWS` in `tools/job-budgets.mjs`). `ci-health` reads its runs for that budget, for
  the job durations, for the flaky tests and for "red on a green run".
- **sccache's server now lives as long as the job** (`SCCACHE_IDLE_TIMEOUT=0`, in the scenario
  steps). At the default of 600 s it stopped in all 8 of those runs, about ten minutes into
  purlis-core's compile, and every crate after that compiled without it.
