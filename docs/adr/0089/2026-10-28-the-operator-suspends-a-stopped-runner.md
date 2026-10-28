# Amended 2026-10-28: the operator suspends a stopped runner

**Built by #2058** on spec #2041's *"a stopped field on Pool and Claim suspends and never
destroys"*. *Decided in implementation, to be confirmed by the person* (D-2058-1 in its pull
request). Item 1 of the amendment of 2026-10-26 (*"reads a runner's sandbox and pod, by name,
and nothing more of them"*) is amended; everything else it says holds.

1. **The operator patches a runner's sandbox, in its operating mode alone.** Upstream suspends
   a runner only through its sandbox's `operatingMode` (`Suspended` ends the pod and keeps its
   volumes); its claim has no such field. So the operator's ClusterRole gains `patch` on
   upstream's `sandboxes`, and it applies one field, `spec.operatingMode`, and only to a sandbox
   its Claim's upstream claim controls by uid: `Suspended` while the Claim or its Pool is
   stopped, `Running` again once neither is, and that only where its own apply suspended it. It
   still never lists, watches, makes, changes or deletes a pod, and holds no `exec`, `attach`,
   port-forward or Secret right. It could already make any pod through the templates it writes,
   so the right adds no reach.
2. **It reads the pod the sandbox records** (from #2056's note): upstream's
   `agents.x-k8s.io/pod-name` annotation where the sandbox carries one, its own name otherwise,
   with the pod still held to being the one that sandbox controls and of the pool's template.
