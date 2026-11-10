# Amended 2026-11-10: the release profile writes line tables into a packed dSYM

**By the operator's ruling of 2026-11-08 on #5692**, carried out in the change that switched the
native crash handler on (#5691): `[profile.release]` gains `debug = "line-tables-only"` and
`split-debuginfo = "packed"`, and keeps `strip = true`. `dev-release` inherits both, so the
channels still differ only in their bundle set and the profile's name.

**Why.** A native crash's Crash report carries its crashing thread's stack as module names and
offsets (ADR 0118, amended 2026-11-10). The Report intake turns them into functions, files and
lines only with each build's debug symbols, matched by debug id. A stripped binary with no debug
info had none to give.

**What it does.** On macOS, rustc writes the line tables, runs `dsymutil` to pack them into a
`<binary>.dSYM` in `target/<profile>/deps/`, which cargo links to from beside the binary in
`target/<profile>/`, and then strips the binary, so what ships carries no more symbols than
before. The two share one UUID. The build copies what each link points at (`cp -RL`): a copy
of the link alone dangles, and the artifact would leave it out. `release.yml`'s build copies
the dSYMs into an artifact of their own, which the `symbols` job alone downloads and uploads to
the intake (#5692); the bundles take the binary and never its dSYM. `line-tables-only` is the
least debug info that still gives file and line: no types and no variables.

It qualifies the record's rejection of symbols in a dev build: a dev build still ships none,
since `strip = true` holds for both channels, and both now write a dSYM beside the binary that
no bundle carries.

**What it costs.** More work for the compiler and the linker, and `dsymutil` on each binary.
That cost is measured on this change's own release run and the nightly profile guard (decision
6); if it pushes either past its runner, the answer is a new amendment, not a quiet revert.
`crates/release-manifest`'s profile test holds the settings.
