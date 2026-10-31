### Fixed

- **A start while a pool's runner is still fetching says to sign in on the pool.** If the runner found
  the profile signed out, the start told you to sign in on this machine, which signed in the wrong
  one. It now names the pool and Sign in on pool, with the pool's own Inbox wait (#4233).
