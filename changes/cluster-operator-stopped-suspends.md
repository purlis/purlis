### Added

- **A cluster operator Pool or Claim can be stopped from kubectl.** `stopped: true` suspends
  every runner of the pool, or the Claim's runner: its pod ends and a kept Claim's volume stays.
  Once `stopped` is false a kept Claim's runner comes back on that volume, paired as it was; a
  fresh Claim's runner starts again with nothing kept. A warm runner made from a pool's earlier
  template is now replaced, so a fresh Claim is never handed one the operator will not vouch
  for. The operator's new right to change a sandbox's mode is narrowed by a second admission
  policy in `policy.yaml`; apply it with the definitions (#2058, docs/cluster-operator.md).
