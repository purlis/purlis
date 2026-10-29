### Changed

- **A cluster operator Claim's runner runs in its requester's own namespace**,
  `purlis-runners-<hash>`, where a role binding gives that person alone the new cluster role
  `purlis-runner-owner` (their pods, `exec` and logs) and a quota, `purlis-person`, caps their
  runners: the chart's new `perPerson.quota`, by default 5 claims, 4 CPUs, 8Gi of memory and
  50Gi of storage. A Claim over it says `OverQuota`. A finalizer, `runners.purlis.dev/runner`,
  removes a Claim's runner when it is deleted, and the statement the operator signs names the
  runner namespace, so a desktop and an operator of this release go together. A Claim made
  before the upgrade gets a new runner there; make it again to free its old one (#2055,
  docs/cluster-operator.md).

### Security

- **Each person's runners on a cluster pool are theirs alone to reach.** Each person's runner
  namespace binds only them to `purlis-runner-owner`, and the new admission policy
  `purlis-runner-namespaces` lets only the operator make a runner namespace, and the operator bind
  only that role, only there, only to that person (#2055, docs/cluster-operator.md).
