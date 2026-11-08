### Fixed

- **Two machines that each remembered something or allowed a host no longer stop the plane's
  save.** A memory index is rebuilt from its memories when the other machine's saves come in, and
  the project's allowed hosts and browser sites merge as sets, so a host either machine removed
  stays removed. The rest of the project's settings file merges setting by setting: one only
  the other machine changed is taken, and one both changed is named in the Saving view, beside
  the file (#5824).

### Changed

- **The project's settings file lists its allowed hosts and browser sites one per line, in
  order,** so two machines that each add one change different lines (#5824).
