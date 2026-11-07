### Fixed

- **The save status in the title bar no longer reads every file twice on a big project.** Its
  check for a merge stopped part-way looked at every tracked file on disk, when git's index
  already says which files are in conflict. On the 200,000-file test project it was 24 s of each
  status on macOS and 1 s on a fast Mac; it now reads the index alone (#5486).
