# Amended 2026-11-01: a runner namespace enforces restricted Pod Security, and a runner has limits

**Ruled by the person on 2026-11-01**, relayed by the dispatcher, on #3824. Item 4 is *decided in
implementation* on #3844. It grants the operator no right.

1. **A runner namespace the operator makes enforces the restricted Pod Security Standard**, by the
   label `pod-security.kubernetes.io/enforce: restricted`, and the policy on runner namespaces
   refuses the operator one without it. It holds a runner's pod to what the template already says.
2. **What this means for a browser pod (#2059).** The Standard admits a `Localhost` seccomp profile,
   so the profile spec #2041 ships fits. It refuses seccomp off, with pod user namespaces or without,
   and adding the capabilities to start another user. So spec #2041's per-pool setting for seccomp
   off needs a ruling of its own when it is built, since the label is per person and not per pool,
   and the host, the chat and the engine are built as three users by separate containers.
3. **A namespace made before this stays as it is**: the operator holds no right to change one, and
   a cluster admin can label it (docs/cluster-operator.md).
4. **A runner's pod has a memory limit and an ephemeral storage limit, and no CPU limit**
   (docs/cluster-operator.md). **A suspended kept runner takes its pool's resources along with its
   pool's image**: this amends D-3982-3, *only the images change*, since a quota holding
   `limits.memory` refuses a pod with no limit, and such a runner could not start again. The policy
   on a Sandbox's changes already allows it, on a Sandbox already `Suspended`. Vouching still leaves
   a container's plain resources out of what it compares.
