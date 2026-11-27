# ADR 0086, amended: the window and felt rows are judged on the benchmark Mac, by detection after merge (2026-11-27)

**Agreed by the operator** in spec #7730's grilling (2026-11-26, 20 decisions); **the tracer
bullet's shape decided in drafting (#7737)**, for the person to accept by merging. The rows'
values do not change, and nothing else in this record moves.

**Why.** Most rows a person feels (L2's switch, L4's frames, L7's reattach, L9, L11 to L15, M6, the
window halves of L1 and L3, L10's real 300 ms) were checked only by hand, on the operator's Mac,
once per release, so a slowdown landing on a Tuesday was found weeks later, with no commit named.
GitHub's shared macOS runners cannot take them over: identical runs' timings swing by up to 46%,
so no row there could ever meet §1's five runs within about 7%. The Linux and xvfb route (#931)
was never tried, and Linux is not where people use purlis.

**The decision.** The window and felt rows are measured on a **benchmark Mac**: one fixed machine
(the operator's M1 Max, 64 GB), a self-hosted runner in a dedicated CI user that logs in by
itself so the real window can be drawn. They are judged **by detection after merge, not by
gating a pull request**: this repository is public, and a workflow a pull request can start runs
its author's code, which must never reach a persistent self-hosted machine
(`tools/no-self-hosted-on-pull-request.test.mjs`). So:

- **The budget run** (`.github/workflows/budget-run.yml`) starts on each push to `main` and
  nightly, and on nothing a pull request, the merge queue or another workflow's run can start.
- **It runs only on the benchmark Mac.** Its runner is the repository variable
  `BUDGET_RUN_RUNNER`, with no fallback: unset, or naming no self-hosted runner, the job does not
  run, so a series never mixes two machines. The Mac's runner group is restricted to the
  benchmark workflows on `main`.
- **One measurement at a time.** The Mac has one runner, so a job of either benchmark workflow
  waits for it. Each workflow's concurrency group keeps one run of main's pushes and the newest
  pending one; a run that started is never cancelled, and the nightly has its own group so a push
  never replaces it. The group is not shared with `outcome-bars.yml`: GitHub keeps one pending run
  per group, so a shared group would cancel one workflow's run for the other's on every push.
- **The stand-in harness only**, on the dev channel's `dev-release` build with the `e2e` seam:
  deterministic, so one day's point compares with the next. Real harnesses stay in the release
  run (#814, narrowed to them).
- **Each result is a point of a `macos-max` series** on the `benchmarks` branch, per row, case and
  renderer arm (`tools/budget-series.mjs`), as the median of the run's samples, carrying the
  commit, the Mac's model, its OS and its screen mode. A hardware or OS change shows as a step.
  **A run whose screen mode is not the pinned one** (looks like 1920×1080, HiDPI, 60 Hz, on the
  HDMI dummy plug) is evidence only: its points go to a data set apart, never into the trend.
  The token that writes the branch is on a GitHub-hosted runner, never on the Mac.
- **A row becomes a gating row** once five consecutive `main` points spread by at most about 7%,
  §1's rule. A gating row regresses on a sustained slowdown past 20%, which ci-health files as a
  ticket naming the commit range (#7730). Until a row gates, it is evidence. Nothing reverts.

**What moves in the budgets table.** Each row the budget run takes names it in "Measured by" as
it starts recording, and keeps *release absolute* against the spec's value; the run adds the
series beside it. The tracer bullet (#7737) records **L2** alone; the nightly set adds the rest.
#931's window and felt part is superseded.

**Rejected.**
- *Gating pull requests on the benchmark Mac.* It would let any fork's code run on it.
- *GitHub's macOS runners.* Their noise is past §1's steadiness rule for every felt row.
- *A fallback to `macos-latest` while the Mac is away.* It would splice two machines into one
  series; an offline Mac is a ticket instead (#7730).
- *Recording from the Mac itself.* The job that writes the repository would hold its token on a
  persistent machine.
