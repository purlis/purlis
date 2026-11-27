# Amended 2026-11-24: a dev build gives third-party crates no debug info

**By the same decision as ADR 0092's lighter dev profile (#7324, CI & release speed):**
`Cargo.toml` gains `[profile.dev-release.package."*"]` with `debug = 0`. Only `dev-release` has
it. `release`, and so a stable build, a build started by hand and the nightly profile guard,
keep their dependencies' `line-tables-only`.

**Why.** The macOS dev build is memory-bound on the three-core, 7.5 GB runner and swaps 2 to 3 GB
(measured in the R7 findings of spec #7324). Line tables for several hundred third-party
crates are most of the debug info the compiler writes and the linker reads, and nothing reads
them back: the dev channel strips its binaries (`strip = true`), and the Report intake needs
file and line only for purlis's own frames.

**What it does.** Cargo's `"*"` package selector matches every package that is not a workspace
member, so the workspace's crates keep `debug = "line-tables-only"` and
`split-debuginfo = "packed"` from `release`, by inheritance. The `symbols` job and
`tools/symbols-upload.mjs` read the packed dSYMs, which still exist and still carry the
workspace's line tables and the same debug id as the binary. Nothing in `release.yml` or the
release-manifest tests reads a dependency's debug info.

**What it costs.** On the dev channel a native crash whose stack passes through a dependency
symbolicates that frame to its module and offset, with no function, file or line, where the
workspace's frames still resolve. A stable build is unchanged. This is a second way the
profiles differ, beside the link-time optimisation that [the amendment of the same day on
`lto = "off"` (#7418)](2026-11-24-the-dev-release-profile-links-without-lto.md) turned off, so
the rule that they differ "in nothing else" now reads: in the profile's name, its link-time
optimisation (off) and its dependencies' debug info (none).

`crates/release-manifest`'s profile test holds the rule exactly: `dev-release` is `release`
plus `lto = "off"` and this one table, and `release` has none.
