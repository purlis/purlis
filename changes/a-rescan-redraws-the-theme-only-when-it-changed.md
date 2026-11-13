### Fixed

- **A busy Mac no longer redraws the window's theme for nothing.** When macOS drops file events
  under a build's load, purlis now compares the theme folders with how they were before, and
  redraws only when one of them changed.
