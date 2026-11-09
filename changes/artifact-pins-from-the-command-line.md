### Added

- **Pin a workspace's pages from the command line.** `purlis artifact pin | unpin | list |
  reorder` keeps a workspace's pinned artifacts in `artifacts/pins.toml`, one pin per line, so two
  people's pins merge cleanly. A LIVE workspace shares its artifact templates and pins with the
  plane, and each person's data files (`data.json`, `data/`) stay on their machine (ADR 0120,
  #5950).
