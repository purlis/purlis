### Added

- **Your profiles on a cluster pool's runner.** A cluster pool you marked *This is my own* now
  lists its profiles on its row in Settings › Pools, as a Docker pool does: Add a profile…,
  Remove, and Sign in, which runs the harness's own login on the runner over your own
  `kubectl exec`. Nothing of this Mac's sign-in goes there. A profile goes to the runner, and
  signs in there, only while the pool is marked and the cluster vouches for its claim at that
  moment. On a kept claim a sign-in outlives the pod; on a fresh claim the row says it lasts only
  for that claim. The pool's chosen profiles are kept as `profiles` in its record, spelled as a
  Docker pool's (#3819).
