### Fixed

- **A pool runner made again keeps where its skill runs were logged up to.** Past 256 kept
  cursors, the window used to let go of the lowest, so the cursors of a runner made again (whose
  log counts from the start) went before those of the runners it replaced, and that runner chat's
  skill runs made while no window was open never reached the studio. The runner kept least
  lately now goes first (#5657).
