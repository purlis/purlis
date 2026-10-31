### Added

- **A cluster Pool can opt in to minting GitHub App tokens in the cluster** (#4204). A cluster admin
  sets `spec.forge` on a Pool (the App's key Secret, an allow-list of repos and `mintInCluster`) and
  names the Secret in the chart's `forge.keySecretNames`, the only Secret the operator may read. A
  desktop records the opt-in as it adds the pool. Nothing is minted yet.
