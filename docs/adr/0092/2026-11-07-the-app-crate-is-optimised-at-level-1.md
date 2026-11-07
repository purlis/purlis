# Amended 2026-11-07: the app crate is optimised at level 1

**By the operator's rulings of 2026-11-06 and 2026-11-07** on #5277, after an A/B measurement on
the hosted runners. `release` gets one per-package override, `[profile.release.package.purlis-app]`
with `opt-level = 1`. Every other crate stays at `opt-level = 3`. `dev-release` inherits the
override with the rest of `release`.

It qualifies decision 7: `opt-level = 3` is inherited by every crate but `purlis-app`, which both
profiles build at 1. Decision 1, as amended 2026-11-06, now also covers this override: the stable
build's profile changes with the dev channel's. The channels still differ only in their bundle
set and in the profile's name. The rest of this record stands.

## Why

The `dev-release` build peaked at about 14.4 GB on the Linux runner, almost all of it one
`rustc`: `purlis_app_lib` at `opt-level = 3`. That fits a 16 GB hosted runner only with a
32 GB swap file added (#5277, `docs/research/build-time-and-memory.md`). An override cannot set
`lto` or `codegen-units` per package in a way that splits that one crate, but it can lower its
`opt-level`.

## What was measured

Runs 42279014678 and 42299963320 on `measure-build-memory`, built the way `ci`'s outcome-bars
job builds (`npx tauri build --features e2e ... --profile dev-release`). The candidates were set
with `--config profile.dev-release.package.purlis-app.<key>=<value>`. Main's settings were
built twice, as `base-a` and `base-b`, so the noise between two identical builds could be seen.

| Linux whole-build peak RSS | Run 42279014678 | Run 42299963320 |
|---|---|---|
| main (`opt-level = 3`, 16 units), twice | 14.45 / 14.46 GB | 14.41 / 14.46 GB |
| `opt-level = 2` | 14.50 GB | |
| `opt-level = 3`, 4 units | 14.51 GB | |
| **`opt-level = 1`** | **6.58 GB** | **6.61 GB** |

The threshold was 10 GB, so only `opt-level = 1` passed, and the order of preference among the
passing ones (4 units, then level 2, then level 1) left it as the choice. Its build also took
16 min 2 s against main's 22 min 43 s and 24 min 40 s.

For `opt-level = 1`, against `base-a`, in the second run (the first run's are in the research
note and agree):

- **Cold start on Linux** (`tools/coldstart-linux.sh`, interleaved), ratio against main:
  1.006 bare, 1.013 under i3, 0.999 under i3 without the session bus. `base-b` against `base-a`
  gave 0.989 to 1.002. The threshold was +5 %.
- **The outcome bars' openings, codex's stand-in**, medians of two rounds:
  - Linux: 437 and 452 ms against 443, 424, 435 and 451 ms for main, about +1 %;
  - macOS: 273 and 213 ms against 272, 288, 218 and 204 ms for main, about −1 %.

  The threshold was +10 %. On macOS the two copies of main differed by 25 %, so the hosted runner
  cannot show a 10 % change there. The operator's run of the `opt-level = 1` app on their own Mac
  opened under the 300 ms budget (2026-11-07).
- **Not measured.** claude's and opencode's stand-ins did not run in the measurement job, on main's
  build as much as on the candidates, so they say nothing about the override. The operator ruled
  codex's bars enough (2026-11-07). The session layer's latency bench builds only
  `purlis-session-protocol`, which the override does not touch (ruling of 2026-11-06).

## What it costs

The app crate's own code, the IPC glue and anything generic instantiated in it, is optimised
less. Whether ThinLTO then optimises purlis-core's code at level 1 too is not settled; the
cold-start and bar numbers above say the difference does not show at the window. The dev channel
carries the change first, for at least three days as the operator's daily app (DF-2), before a
stable build does.

Going back is removing the override, which needs the swap file again or a larger runner.
