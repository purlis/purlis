# `purlis-core` stays one crate for the release build, and the app's leaf layer is not split off

**Proposed 2026-11-24** for the person's acceptance, from the grill on #7324 (R8). Inputs:
research/r8-split-note.md, R0 and R7. Nothing here moves code.

## Decision

1. purlis-core is not split to shorten the release build. The foundation crate (purlis-core-base,
   seam C) and a theme or pools crate (seam D) are not scheduled. They reopen only if a measured
   complaint about core's incremental cargo check or cargo test shows up, or if R0's timeline,
   re-measured on the shipping build shape, puts core on the critical path.
2. The app's upper leaf layer (68 modules, 24k of 218k lines, seam B) is not split off, even as a
   step towards a larger split.

## Why

- 254 of core's 321 modules (about 628k lines) are one strongly connected component. Removing the
  15 most-connected modules still leaves 301k lines; 91 modules stay connected after 25. A split
  needs cycles untangled first, and that is the cost, not the cut.
- In R0's macOS timeline core's front end is 293 s and its codegen ends at 2,141 s, before the
  app lib's (2,609 s). A core split gains 0 to 1 min. A layered split does not parallelise front
  ends either.
- Seam B is a layered split of 11% of the app: about 50 s of codegen overlap at best.
- R0 is one run per setting on a build shape (two passes, thin LTO) that no longer ships. R7's
  identical macOS runs differ by 7 to 29 min. A gain under 10 min cannot be told from that noise.

## Not decided here

Whether the app crate splits, and how. That waits for the LTO-off re-measure, a macOS contention
experiment, and a macOS profile with a hub-closure inventory. The crate-family ADR and the ADR 0092
amendment are written then, not before.
