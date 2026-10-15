### Added

- **Chats drive a real browser.** Every harness's chat gets purlis's `browser_` tools: a
  snapshot of a page with a reference for each element, clicks, typing, drags, waits and a
  screenshot the model sees as an image, on a browser purlis runs and no script the model
  writes. A chat's first browser tool asks you once what its browser may reach: this project's
  sites and its dev servers, also the open web, or Keep off (#2013).

### Changed

- **A sandboxed chat no longer offers its harness's own browser.** A Claude Code chat purlis
  sandboxes is denied its Chrome integration; the browser a chat drives is purlis's (#2013).
