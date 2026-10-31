### Added

- **A chat on a pool that opts in to minting gets its GitHub token from the cluster**, so `gh` works
  with the desktop closed and nobody is asked about each run. The token covers only the repos on both
  the Pool and the Claim, stays in the runner's memory, and is revoked as the chat ends. A refused mint
  goes to the desktop and the person's Ask. Pools that do not opt in are unchanged (#4207).
