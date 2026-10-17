# ADR 0086, amended: Linux cold start is held to main, measured in the same job (2026-10-13)

**Decided in implementation (#1866, D-1866-1; flagged for the operator as an amendment of row
L6).** The Linux cold start on a pull request is held to main's build, measured in the same job,
and the 2 s limit is reported there, not gated.

**Why.** §1 allowed L6 as an absolute timing in CI only because its budget sat far above its median.
On Ubuntu runners it no longer does. Across train 84's and 85's runs, with the launch unchanged, the
medians went from 874 to 2537 ms, and two of train 84's runs went red on it, each a rerun. Within
one job, the five launches sat within a few percent of each other, apart from a slow one now and
then. The variance is the runner's speed, which moves the whole job, and not the launches. More
samples and a trimmed statistic would not help, since every sample shares that speed. A ratio over a
stored value, like §1's first rule, compares two runners. The SC-16 amendment already showed what
cancels it: main and the change launched in turn on one runner.

**What changes for L6 in `app builds`:**

- **On a pull request**, the job also builds main's app: the merge commit's first parent, in the
  same target directory. Each case (bare, i3, i3 with no session bus) launches each build once,
  discarded, then seven rounds of one launch of each, in alternating order, each on a fresh
  profile. A pass's ratio is the median of its rounds' ratios, the change over main.
- **The line is 1.15.** A pass past it asks for a second pass. The case fails only when both are
  past it. One past and one not is reported as unconfirmed, and passes.
- **Why 1.15 and not SC-16's 1.20 with its "all rounds but one" rule.** It was set by resampling
  the launches the runners recorded, as main against itself and as a change 30% slower, or with
  30% of main's median added. At 1.15 with two passes, no false red came up in 20,000 trials, and
  the 30% change was caught in over 99%. SC-16's rule caught it in 65% to 90%: one slow launch of
  main's in a round is enough to keep that round under the line.
- **The 2 s limit is reported, not gated**, beside the ratio in the job's annotation.
- **No baseline, the limit gates.** If main's app does not build, the case falls back to V39's
  absolute rule (median of five ≤ 2 s, at most one past 2.5 s). The gate never goes quiet.
- **On a push to main and on a nightly, the cold start is evidence.** The pull request already held
  the same code to main. A miss is a warning, and the job does not fail on it. This applies the
  2026-10-04 amendment's rule for evidence.
- **Cost.** A second debug build of the app, reusing the first one's dependencies, and nine more
  launches for each case on every pull request's Ubuntu `app builds` job.

**What it gives up.** A slow drift of less than 15% per pull request is not gated on Linux. The
absolute figure is still reported on every run, and macOS keeps its release absolute row (#814).

`tools/coldstart-gate.mjs` decides (`coldStartAgainstMain`), and its tests use the runners' own
recorded launches. `tools/bench.mjs --baseline-app` runs the rounds, and
`tools/coldstart-linux.sh` passes `BASELINE_APP` and `EVIDENCE_ONLY` through.
