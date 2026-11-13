### Fixed

- **Pool setup on a cluster no longer claims your runner before the cluster will accept it.**
  Right after Install, purlis's policy on claims takes about half a minute to come into force,
  and the cluster operator refuses for good a runner claimed before then, which left the Pool
  step saying "Claiming your runner" for minutes. The Pool step now waits until the policy is in
  force, says how long that will take, and offers Add the pool here once it is.
