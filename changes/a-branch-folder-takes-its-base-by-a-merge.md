### Added

- **A sandboxed chat can merge its base into a branch folder.** `purlis worktree update --merge`
  has the app run the merge, so no commit is rewritten and a plain `purlis worktree push` follows,
  with no line typed by the person. A conflict stops it and names the files, with `--continue` and
  `--abort`. `--take-base-baselines` gives a conflicted `.png` under a `__screenshots__` folder the
  base's copy, since CI redraws those; every other conflict still stops (#5253).
