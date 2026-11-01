# Amended 2026-11-01: a policy's time in force, and a policy on what the operator writes

Item 1's right is **ruled by the person on 2026-11-01**, relayed by the dispatcher, on #3854. The
rest is *decided in implementation* on #3854 and #3754.

1. **The operator gains `get` on the policies `purlis-claims-use-their-pool` and
   `purlis-runner-namespaces`, by name.** It already had this on `purlis-forge`. It reads their
   creation times and nothing more.
2. **A policy is in force from 30 seconds after the later of its and its binding's creation
   times,** with its binding's `generation` 1. A binding changed since it was made puts nothing in
   force until it is made again: a cluster keeps no time for the change.
3. **`purlis-operator-writes-only-a-runners-shape` holds what the operator's service account
   writes of `SandboxTemplate`, `SandboxClaim` and `SandboxWarmPool`**: in a runner namespace alone,
   and a template's pod in purlis's runner shape (docs/cluster-operator.md). It fails closed, and is
   kept on uninstall. An allow-list of runner namespaces stands in for a list of system namespaces
   and a chart value naming more, since the operator writes these nowhere else.
