### Fixed

- **A pool's runner removed outside purlis says so, and is made again on your word** (#4149).
  When a Docker pool's container was removed outside purlis (a cleanup, say), every chat start
  there failed with "No such container" and purlis tried to link again forever. Now purlis tells
  that apart from a runner that is only away, stopped or suspended, stops trying, and says it
  plainly on the pool's row and when a start is refused, with **Make the runner again**: a runner
  made from the pool's own image and limits, paired, and given the pool's profiles again. It
  takes up the old home volume where that is still there, with its sign-ins; where the volume went
  too, it says so, and the profiles are signed in again.
