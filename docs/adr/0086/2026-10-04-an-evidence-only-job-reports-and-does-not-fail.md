# ADR 0086, amended: an evidence-only job reports and does not fail (2026-10-04)

**Decided by the dispatcher (D-ME-b, operator offline, 2026-10-04).** An *evidence only* row is
reported, never gating (as the SC-16 amendment says). This amendment says what that means for the
job that measures it. **A budget it misses goes into the job's summary and a warning, and the job
does not fail for it.** It fails only when the run itself breaks: a wrong answer, a panic, a
timeout. `continue-on-error` is not enough. It keeps the workflow green, but the job's own check
run is still red, and GitHub shows the commit as red. A `main` that is red for a known runner cost
teaches the repo to ignore red.

**Why now.** `stress.yml`'s *search at scale* holds G2–G4 (`docs/spec.md`). It failed on every
macOS run on `main` since it was added (ten runs on 2026-10-04), and passed on every Ubuntu run with
the same code. At 100,000 files,
the macOS runner took 8–12 s for a first ⌘P find (budget 3 s; 0.43 s for the same find in a second
session of the same run) and 44–98 ms for its slowest ⌘P keystroke in the runs read. Writing the repo took 5–7 times as
long as on Ubuntu, and the whole-repo scan took 3.5–13 times as long. The operator's machine does
the same first find in 0.19 s warm and 1.7 s cold. Those are the runner's file system and its three
cores. They are not purlis's cost, and §1 already keeps a single-sample timing like "the worst
keystroke" out of CI's absolute rows.

**What changes:**

- G2–G4 are *release absolute* by hand on the operator's machine:
  `search_is_measured_on_large_repos` fails there, as before, when a budget is missed.
- In CI they are *evidence only*: `CHARTER_MEASURE_BUDGETS=report` prints each miss as a
  `FM-12 | budget missed` line, and the job turns those into warnings, a job summary and an
  artifact.
- A status in that run also prints gitoxide's own counters for the compare (files read, entries
  racily clean, entries whose stat changed) and `git status`'s time on the same repo. A slow status
  then says whether the disk was slow or a stat had stopped matching the index. A reader that never
  writes the index pays the second on every read (#1153's follow-up).
- Any other evidence-only job that fails on a budget follows this rule when it is next touched.
