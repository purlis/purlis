### Fixed

- **A chat's browser reads a page whose script cut an emoji in two.** Half of the emoji reads as
  �, and the browser goes on; a page's very large network request no longer ends it either
  (#2745).

### Changed

- **Every browser's end is in the log.** The window's log says who ended a browser, why, and its
  engine's exit code or signal. Each engine's own log is kept beside its browser's folder,
  `<data>/browsers/<id>.engine.log`, the newest 20 of them (#2745).
