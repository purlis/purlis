### Fixed

- **Signing a profile out of a pool says "signed out" only once the runner confirms it.** Where
  its harness's status check can't say, Settings › Pools now tells you purlis could not confirm
  the sign-out, and to look at the pool's row.
- **A sign-in on a runner is tighter about where the browser's answer goes.** The answer reaches
  only the login that sign-in started, on the runner it started on, and once. Cancelling while
  the runner fetches its harness no longer goes on to start the login.
