### Fixed

- **A paste in a chat no longer quits purlis on macOS.** When macOS raised an error of its own
  while it handled a key (seen on Cmd+V under macOS 27), purlis quit with "purlis quit
  unexpectedly". macOS now reports the error and purlis keeps running. A fault in purlis itself
  still ends the app, as before.
