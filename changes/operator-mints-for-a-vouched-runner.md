### Added

- **The cluster operator can mint a GitHub App token for a vouched runner.** On a Pool opted in to
  minting, a runner proves its link key and gets a token covering only the repos on both its Pool
  and its Claim. The operator asks the cluster before each mint whether the requester may still use
  the Pool, and logs every mint and refusal. No chat uses this yet (#4206).

### Changed

- **Pointing a Pool at a GitHub App key takes `use` on that key** (`forgekeys`, by the Secret's
  name). A Pool or Claim made before the new policy `purlis-forge` settled must be made again to
  mint, and `use` on a minting Pool must be granted to the person by name: a right held only through
  a group is refused (#4206).
