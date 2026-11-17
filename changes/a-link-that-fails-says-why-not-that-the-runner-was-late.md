### Fixed

- **A pool's runner that would not link says why, and where to look.** After a pool's machine
  restarted, a start could fail with "the runner did not answer in time" while the real cause
  went unsaid, and the runner logged a handshake that failed against another host. purlis now
  waits out the link's own deadlines, says why the link is down with "Check pool … in Pool setup",
  and the runner logs that the other end went away rather than a changed key.
