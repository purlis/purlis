### Changed

- **The cluster operator changes only upstream objects it made.** An agent-sandbox object
  under the name it would use that it did not make is left as it is, and the Claim says
  `UpstreamObjectInTheWay`, naming it, until it is removed or renamed. Its ClusterRole gains
  `get` on `sandboxclaims`, `sandboxtemplates` and `sandboxwarmpools`, to read each by name
  first (#2258, docs/cluster-operator.md).
