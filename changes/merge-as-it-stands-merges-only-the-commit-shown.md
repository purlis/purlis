### Fixed

- **Merge as it stands merges only what you were shown.** If, between the fresh look and the
  merge itself, a save pushed a new commit onto the pull request or its checks failed, the
  merge is now refused with "The request changed since you looked" instead of going ahead.
  Look at it again and decide on what is there (#1978).
