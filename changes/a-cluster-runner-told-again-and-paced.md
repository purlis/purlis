### Fixed

- **A cluster runner comes to use its Pool's mint port without being claimed again.** Its desktop
  tells it where to ask at each link, so a Pool that opts in later, or a runner stopped through a key
  change, works from the next link. A chat's start also waits a few seconds for its repos to reach
  the Claim, so its first token covers them.
- **A chat that keeps running a refused tool on a cluster runner no longer keeps asking you.** Its
  runner asks the operator less often, and once you have refused such a run, it waits before asking
  you again and says so. Runs on a cluster's token are now recorded as from the cluster.
