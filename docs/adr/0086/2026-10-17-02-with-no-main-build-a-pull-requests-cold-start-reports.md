# ADR 0086, amended: with no main build, a pull request's Linux cold start reports (2026-10-17)

**Decided in implementation (#2388), accepted by the operator as an amendment of the 2026-10-13
amendment.** Amends `2026-10-13-linux-cold-start-is-held-to-main-measured.md` only in its "No
baseline, the limit gates" line. On a pull request whose job could not build main's app, and
whose main is not green on main either, the Linux cold start is measured as before, against
V39's absolute rule (median of five within 2 s, at most one past 2.5 s), and **reported, not
gated**. The limit itself does not move.

**Why.** The record read for #1866 (524 pull request runs, 2026-10-14 to 2026-10-17) put that
rule against unchanged launch code: its median missed 2 s in 39 of 1,572 cases, and in 13 of 306
on pushes to main, on runners whose medians went from 838 to 2611 ms. That is the runner's speed,
and on this path nothing in the job cancels it. Gating it gives a false red to a few percent of
cases, three cases a run, on exactly the pull requests that come when main is broken: the ones
fixing it. The path is taken only when main's app does not build in the job, and a main that does
not build is red in its own jobs already, so the gate is not what says so.

**Only when main is broken on main.** main's app is built in the pull request's own job, in the
environment the change set up. So a change can break main's build there alone: a toolchain or
Node bump, a dropped system package, a build script that writes the job's environment or the
shared target directory. If that went ungated, a change could switch its own gate off without
anyone meaning it to. So when main's app does not build in the job, the step reads main's own
`app builds (ubuntu-24.04)` check on that commit (the merge commit's first parent) with the job's
token (`checks: read`):

- **not green there** (failed, cancelled, still running, or no such check): no baseline, as below;
- **green there**: the step fails, with an error annotation that says main built on main but not
  in this job, so the change broke main's build here;
- **the check could not be read**: the step fails too, because nothing then says main is broken.

`withoutMainsBuild` in `tools/coldstart-gate.mjs` decides, and `tools/coldstart-no-baseline.mjs`
is the step's script.

**What a run on this path says.** The step that builds main's app warns that it did not, and
names why, with main's commit and its state on main, in an output (`no-baseline`). Each
cold-start case then leaves a **warning**, whether the limit was met or missed, that starts "no
baseline: main's app did not build in this job, nor on main (<commit>: <state>), so nothing is
gated" and gives the median against the limit as "reported, not gated". So the run says on its
face that it went ungated. A case that could not be measured is a warning, as on any run that
reports. This change's own build leaving before its first frame still fails
(`tools/coldstart-gate.mjs`).

**Rejected.**

- **Hold the limit with a margin taken from the record** (say, 2.6 s). It loosens a felt-speed
  budget to fit a runner, and still fails in proportion to how slow the runner was.
- **Build main from the merge base instead, or from a stored binary of the last green main.** The
  merge base is not main: the ratio would carry every commit main took since, each already held
  to 1.15 against its own parent, so they compound. A stored debug binary is about 500 MB a push
  to main, kept for a path that comes up when main is broken. Both still need a last fallback for
  when that does not build either, and this is it.

**What it gives up.** A pull request that lands while main does not build is not held to a cold
start at all. A regression it brings becomes main's baseline for the pull requests after it; what
shows it is the absolute figure, reported on its own run and on its push to main. The gate
against main was never meant to catch a drift of that kind either (the 2026-10-13 amendment's
"What it gives up").
