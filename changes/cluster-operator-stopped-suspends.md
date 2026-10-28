### Added

- **A cluster operator Pool or Claim can be stopped from kubectl.** `stopped: true` suspends
  every runner of the pool, or the Claim's runner: its pod ends, a kept Claim's volume stays, and
  it runs again, paired as it was, once `stopped` is false. A warm runner made from a pool's
  earlier template is now replaced, so a fresh Claim is never handed one the operator will not
  vouch for (#2058, docs/cluster-operator.md).
