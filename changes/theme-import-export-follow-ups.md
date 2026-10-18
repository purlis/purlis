### Added

- **`purlis theme export` takes a theme folder by its level id** (`machine:<folder>`,
  `project:<folder>`, `workspace:<workspace>:<folder>`), as `purlis theme check` does, and never
  exports a farther level's folder in its place (#2612).

### Changed

- **Theme folders hold only names every platform can.** A theme folder or a file in it named as
  a Windows device (`CON`, `NUL`, `COM1`, …) or ending in a dot or a space is refused with a
  sentence, on every platform, in import, export and the theme folder reader (#2588).

### Fixed

- **Checking a staged import leaves no font registered**: the stage is read only to check, so
  nothing of it is served after it moves into place or is removed (#2587).
