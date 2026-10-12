### Added

- **`purlis doctor` checks each workspace's generated layer.** The `workspace layer` row, which
  said "not checked", now compares the settings purlis generates into every workspace folder,
  and into every clone and worktree in it, with what the project's own settings say today,
  without writing anything. A file that is missing or stale is named with `purlis workspace
  reinit --all`; one purlis did not write, cannot read, or cannot confirm as its own is named
  with what clears it instead (#994, #998).
