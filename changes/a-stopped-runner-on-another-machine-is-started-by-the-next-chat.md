### Fixed

- **A pool's runner stopped by its machine's restart is started by the next chat**, as a
  suspended one is, on a pool reached over SSH too. You no longer have to run `docker start`
  there by hand.
