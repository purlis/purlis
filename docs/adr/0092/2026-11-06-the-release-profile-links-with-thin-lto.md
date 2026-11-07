# Amended 2026-11-06: the release profile links with thin LTO across sixteen units

**By the operator's ruling of 2026-11-06,** when the `v0.5.0` tag could not be built: "Release
= dev-release's link".

**What happened.** From 2026-11-02 the nightly profile guard (decision 6) failed every night
(#4833), and so did the `v0.5.0` stable build (run 42092625026). Built with `release`
(`lto = true`, `codegen-units = 1`):

- the macOS job reached its 90-minute limit with the app still compiling: 33 minutes of setup,
  then 57 minutes in *Build the app*, cut off;
- the Linux runner was shut down mid-link, at the same point each night, which is what a hosted
  runner does when it runs out of memory.

The dev channel's `dev-release` (thin LTO, sixteen codegen units) built on the same runners the
same days, and every dev build since 2026-10-07 has shipped with it.

**The decision.** Decision 1 ("the stable build keeps its profile") is amended: `release`
links as `dev-release` does, `lto = "thin"` and `codegen-units = 16`. `opt-level = 3`,
`panic = "unwind"` and `strip = true` stay as they are. `dev-release` still exists and still
inherits `release`, so a build still says which profile made it (decision 9), and the channels
now differ in their bundle set and in that name alone.

**What it costs.** A fat LTO pass over one unit can make a binary somewhat smaller and a little
faster than thin LTO. That difference has not been measured for purlis, and the dev channel has
run with thin LTO for a month without a slowness anyone has reported. Going back to fat LTO is a
new amendment, made when the build fits the runners again, for example on larger runners.
