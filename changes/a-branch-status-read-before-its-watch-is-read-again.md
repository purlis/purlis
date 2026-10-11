### Fixed

- **A file an agent writes just as you open a branch's files gets its change mark.** The
  explorer read what the branch changed at the same moment it asked for the branch to be watched,
  so a file written after that read and before the watch held was drawn unmarked until something
  else moved in the branch. What a branch changed is now read once its watch holds, or after two
  seconds and once more when the watch holds (#1107).
