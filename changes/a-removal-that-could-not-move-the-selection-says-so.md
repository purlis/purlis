### Fixed

- **A workspace removal no longer claims a reset it could not make.** Where purlis could not
  write the new selection after removing the workspace you were on, it now says so and exits
  with a failure, instead of "Active workspace reset to …" (#2103).
