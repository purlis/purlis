### Fixed

- **A session host that stops answering no longer piles up secret asks' steps in memory.** At most
  512 wait on it; a step past that is not written to the event log, and takes its ask's later
  steps with it, as a raise the log refused does. The chat's trace keeps the whole of the ask
  (#5618).
