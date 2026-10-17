### Changed

- **Deleting a cluster operator Pool stops no runner.** A Claim whose runner was asked for
  keeps it until the Claim is deleted, and says `PoolDeleted` with the runner's own state; a
  pool's template and warm pool in a namespace now belong to its Claims there (#2259,
  docs/cluster-operator.md).
