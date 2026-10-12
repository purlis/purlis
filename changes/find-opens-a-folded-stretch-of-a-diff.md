### Fixed

- **Find in a diff opens the folded stretch its match is in.** The diff view folds long
  unchanged stretches, and a match inside one used to be counted and selected but stay hidden.
  Finding it now unfolds that stretch on both sides, as a click on the fold does, and leaves
  the other folds as they were (#1797).
