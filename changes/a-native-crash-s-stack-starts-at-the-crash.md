### Fixed

- **An abort's Crash report starts at the crash, not in the crash handler.** The crashing
  thread's stack of a crash that arrived as a signal now begins at the frame that raised it, so
  every abort is no longer titled and grouped as one crash in the handler's wait (#5691).
