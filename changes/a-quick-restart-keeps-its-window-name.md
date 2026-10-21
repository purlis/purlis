### Fixed

- **One window link at a time holds a window's name, and a quick restart still gets it.** The
  desktop's host refuses a second link a name another of its relayed links holds, and a window
  that restarted before its old link ended asks again a moment later. A window-names file too
  long to read is started again rather than leaving every link unnamed (#2744).
