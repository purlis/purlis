### Fixed

- **A native crash on macOS always leaves its Crash report.** About one crash in 400 left none:
  the app could reach its crash handler while the handler was still starting, and the handler
  took that for the app ending and quit, so the crash that followed found nobody to write it
  down. The app now waits until the handler is ready before it connects (#6909).
