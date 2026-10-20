### Fixed

- **A page a script retitles is listed under its new title.** Listing a chat's browser's pages
  reads each page's title from the page itself, where Chrome did not always say it had changed
  (#2753).

### Changed

- **`purlis serve` says what purlis does on its standard error.** A runner's host now writes the
  core's `info` lines there as well as its warnings, redacted as the app's log is, so the
  container's log or the service's journal shows an engine update or a browser's end (#2753).
