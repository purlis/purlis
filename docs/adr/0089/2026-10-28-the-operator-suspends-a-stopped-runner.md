# Amended 2026-10-28: the operator suspends a stopped runner

**Built by #2058** on spec #2041's *"a stopped field on Pool and Claim suspends and never
destroys"*. *Ruled by the person 2026-10-29*, as narrowed by the admission policy below
(D-2058-1 in its pull request). Item 1 of the amendment of 2026-10-26 (*"reads a runner's
sandbox and pod, by name, and nothing more of them"*) is amended; everything else it says holds.

1. **The operator gains `patch` on upstream's sandboxes.** Upstream v1.0.6 suspends a runner
   only through its Sandbox's `spec.operatingMode`: `Suspended` has upstream end the pod and
   keep the volumes. Its SandboxClaim has no such field, and its other lifecycle fields delete
   the Sandbox, a kept Claim's volume with it. So the operator's ClusterRole gains `patch` on
   `agents.x-k8s.io/sandboxes`, the smallest verb that does it.
2. **What RBAC grants is wider than what the operator uses.** A role cannot name a field or an
   owner: as granted, the right is every field of every Sandbox in the cluster, including
   Sandboxes other users of agent-sandbox made without any SandboxClaim. The operator could
   already have upstream start any pod in any namespace, through the templates and claims it
   writes; what the right adds is the power to change Sandboxes it did not make.
3. **An admission policy narrows it in the cluster.** `purlis-operator-changes-only-a-sandboxs-mode`,
   installed with the policy on Claims (`deploy/policy.yaml`, failure policy `Fail`), refuses
   any update by the operator's service account to a Sandbox no SandboxClaim controls (any
   SandboxClaim's, purlis's or not, may), and any
   that changes a field of its spec but `operatingMode`, or its labels, annotations, owner
   references or finalizers. The operator holds no right to change admission policies or their
   bindings, so it cannot lift it. A test holds the policy's list of spec fields to the pinned
   upstream's definition; a cluster serving another upstream version may have fields it does
   not hold. It guards against a mistake in the operator or its token used as it is; a hardening
   of what the operator's own templates and claims may hold is #3754.
4. **What the operator does with it.** It applies `spec.operatingMode` alone, to the Sandbox
   its Claim's upstream claim controls by uid, with that Sandbox's uid as a precondition:
   `Suspended` while the Claim or its Pool is stopped, and `Running` again once neither is,
   where its own apply suspended it (best effort: who last applied a field is the client's to
   name), and never while its Pool is deleted. It never lists, watches, makes, changes or deletes a pod itself, but a suspend has
   upstream end the runner's pod; it still holds no `exec`, `attach`, port-forward or Secret
   right.
5. **It reads the pod the sandbox records** (from #2056's note): upstream's
   `agents.x-k8s.io/pod-name` annotation where the sandbox carries one, its own name otherwise,
   with the pod still held to being the one that sandbox controls and of the pool's template.
