### Changed

- **The cluster operator changes only upstream objects it made.** An agent-sandbox object
  under the name it would use that it did not make is left as it is, and the Claim says
  `UpstreamObjectInTheWay`, naming it, until it is removed or renamed (#2258,
  docs/cluster-operator.md).
