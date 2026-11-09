### Security

- **A runner whose home is older than its Pool's GitHub App key gets no token.** The cluster
  operator marks a runner's upstream claim, as it makes it, when its Pool names a key, nobody can
  change that mark after, and the operator mints only for a runner whose claim carries it, so a
  kept volume from before the key is never a minting runner's home. **Upgrading: a kept runner
  made before this version is refused tokens until its Claim is made again**, and so is a fresh
  one still running from before, until its Claim is made again or its runner goes idle and runs
  again (`docs/cluster-operator.md`).
