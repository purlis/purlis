### Fixed

- **A page settles after a frame of it goes.** When a frame is removed while something it asked
  for is still loading, a chat's browser stops counting that load and waits only a quiet while
  from the frame's removal, rather than its whole settling time (#2841).
