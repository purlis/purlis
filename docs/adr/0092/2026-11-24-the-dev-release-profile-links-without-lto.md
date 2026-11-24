# Amended 2026-11-24: the dev-release profile links without LTO

**By the operator's ruling of 2026-11-24 on [#7418](https://github.com/purlis/purlis/issues/7418)**:
`[profile.dev-release]` sets `lto = "off"`. `[profile.release]` keeps `lto = "thin"`. Nothing
else in either profile moves.

**The rule now.** The two channels differ in **optimisation profile, bundle set and link-time
optimisation**, and in nothing else. This amends the amendment of 2026-11-06, which made them link
alike: `dev-release` is `release` with `lto = "off"`, and a test holds it to that one line
(`inherits = "release"` and `lto = "off"`, with no restated copy of any `release` setting).
`opt-level = 3`, `panic = "unwind"`, `strip = true`, the debug info, `codegen-units = 16` and the
app crate's `opt-level = 1` are `release`'s, by inheritance.

**Why.** R7 measured it ([run 45137164155](https://github.com/purlis/purlis/actions/runs/45137164155)):
`dev-release` on the one-cargo-run job with three LTO settings, each twice per platform, all 12
jobs green. Compile step, minutes:

| | thin (before) | off |
|---|---|---|
| Linux | 32.2 / 31.2 | 24.0 / 18.7 |
| macOS | 114.6 / 107.2 | 111.2 / 91.3 |
| macOS dsymutil | 10.9 / 9.8 | 1.5 / 1.4 |

That is about 10 minutes on Linux and 10 to 20 on macOS. The macOS runner is memory-bound, not
LTO-bound: every variant swapped 2 to 2.8 GB on its 7.5 GB, and `purlis-core` alone, which does no
LTO, took 39 to 64 minutes there against 5 to 8 on Linux. Most of what `off` wins on macOS is
`dsymutil`, which packs the line tables the profile still writes: about 1.5 minutes against about
10, because with thin LTO it has far more to pack. Cold start p50 showed no cost on either
platform (Linux 1070 to 1491 ms against 1564 to 1786, macOS 4271 to 5053 against 4759, inside the
runs' noise). The app is about 8% larger. It does not bring macOS under 90 minutes by itself; the
swapping runner sets that floor, and paid runners were ruled out.

**The guard.** The nightly profile guard builds `release`, with thin LTO, and still does. A fault
that only thin LTO shows (a link failure, a miscompile that only a cross-crate pass exposes, a
symbol that LTO drops) is caught by that build every night, before any stable tag, and a dev
publish no longer shows it. The guard's test is unchanged.

**What it costs.** A dev build is slightly slower and larger than a stable one, which was already
true of it and is already accepted: it is for trying a change, never for measuring one, and every
performance budget is measured on `release` (ADR 0086). The one new thing is that a dev build and
a stable build no longer share a link, so a defect in LTO's pass can no longer show in a dev build
first. The guard is what answers that.

**A two-way door.** Going back is deleting the `lto` line of `[profile.dev-release]` and putting
`codegen-units` and `lto` back in the test, which is a new amendment.
