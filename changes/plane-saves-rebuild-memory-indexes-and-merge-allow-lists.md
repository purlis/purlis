### Fixed

- **Two machines that each remembered something or allowed a host no longer stop the plane's
  save.** A memory index is rebuilt from its memories when the other machine's saves come in, and
  the project's allowed hosts, browser sites and dispatch grants merge as sets, so a host either
  machine removed stays removed. The rest of `charter.toml` merges setting by setting: one only
  the other machine changed is taken, and one both changed is named in the Saving view, beside
  the file (#5824).

### Changed

- **`charter.toml` lists its allowed hosts, browser sites and dispatch grants one per line, in
  order,** so two machines that each add one change different lines (#5824).
