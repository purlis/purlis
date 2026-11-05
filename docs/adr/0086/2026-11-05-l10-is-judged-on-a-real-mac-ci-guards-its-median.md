# ADR 0086, amended: L10 is judged on a real Mac, and CI guards its median (2026-11-05)

**Ruled by the person** (2026-11-05, #4351). L10's limit stays 300 ms at the p95 of 20. It is
judged on a real Mac, on a `dev-release` build, as part of the person's real run.
`outcome bars` keeps a coarser guard against regressions, with no pass or fail at 300 ms. Nothing
else in this record moves.

**Why.** On a hosted macOS runner, the same code's median opening ran from 214 to 649 ms between
runs, so its p95 cannot say whether the app meets 300 ms. Quieting the runner did not change that.

**The guard.** On macOS CI, each harness's median over its 20 openings stays under 800 ms
(`OPEN_GUARD_MEDIAN_MS`, `app/e2e/outcomeBars.ts`). That is about a quarter above the worst seen
on the same code, so a slow runner does not turn it red. Most runs before #4351's fixes measured
880 to 2,300 ms, and a return to that turns it red. It does not catch a small regression, or one
measured on a fast runner: runs before the fixes also measured 370 to 850 ms. The p95 is still
said on CI, marked as not judged.

**Linux.** No limit yet. `outcome bars` on Linux was cut off by a runner shutdown in each of the
five latest runs on main read for this, so no opening of the current code has been measured there
to set one from. It says its median where it does measure one.

**What it is not.** The judgement "L10 is met" comes only from the real Mac's run. That run reads
`CI` as unset; `CI=true` is what selects the guard.
