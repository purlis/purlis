### Fixed

- **`purlis save` no longer fails with merge-tree's usage text when the remote has moved.** The
  replay of a save's own commits asked `git merge-tree` for `--merge-base`, which only git 2.40
  has, so Apple's git 2.39 refused it and the save said "the rebase onto the remote did not go
  through". It now works on any git with `merge-tree --write-tree`, and a git that refuses an
  argument purlis passed is reported as purlis's own fault, not as a failed rebase (#7627).
