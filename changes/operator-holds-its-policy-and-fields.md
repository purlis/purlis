### Changed

- **The cluster operator acts on a Claim only while its policy is the one it ships.** It reads the
  policies on Claims, runner namespaces and App keys, and watches the policy on Claims, comparing
  each with the copy it was built with. While one says anything else, Claims say
  `PolicyNotInForce`; they go on once it is installed again. Its ClusterRole gains `list` and
  `watch` on those three policies, by name.
- **Only the cluster operator makes upstream's objects in a runner namespace**, and a quota or
  network policy it applies there is written over whole when a field another manager added
  survives its apply.
