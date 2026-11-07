### Fixed

- **A runner chat's skill runs reach the Skill studio after the window was closed.** The window
  keeps, for each pool runner and project, how far it logged the runner's skill runs, and reads
  on from there when it links again, so runs a runner chat made while no window was open are
  logged once each. A run's version is the one the runner ran, not the one on this machine by
  then. Behind the Skill studio's flag (#4219).
