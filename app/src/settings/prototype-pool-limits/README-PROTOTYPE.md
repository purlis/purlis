# PROTOTYPE: a pool's limits page

Throwaway. It lives on branch `prototype/pool-limits` and never merges to main.

## The question

Which layout should a pool's limits page have? The decisions of the pool settings grilling
(2026-10-31, Q1 to Q16) are fixed. Variants:

- **A:** one simple list of limits, plus Advanced.
- **B:** limits grouped by what they affect: Code, Sign-in, Runner, plus Advanced.
- **C:** one list, each limit beside the runner's live use, plus Advanced.

## Open it

Double-click `PROTOTYPE-artifact.html` in this folder. It needs no build. Or:

```sh
cd app && pnpm storybook
```

Then open **PROTOTYPE › Pool limits**. The bar at the bottom switches the variant, the pool
(Docker or cluster) and the error's suggestion. Everything is fake and held in memory.

## Placeholders, not decisions

- The defaults for what a profile may send (256 MiB, 20,000 files): the real ones come from
  measurement.
- A Docker pool's highest allowed in the suggestions (4 GiB, 2 h). The decisions only name a
  highest allowed for cluster pools.
- What Replace runner now says happens to running chats and the home.
- The fixed limit's example (checks read at most 1 MiB of output).
