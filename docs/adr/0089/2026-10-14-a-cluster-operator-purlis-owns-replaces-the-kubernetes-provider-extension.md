# Amended 2026-10-14: a cluster operator purlis owns replaces the Kubernetes provider extension

**Ruled by the operator on 2026-10-14**, in the grill on the browser program and the runner
placement spec (#2041): purlis owns a Kubernetes operator, self-hosted and fully free, built on
kubernetes-sigs/agent-sandbox, in Rust, with pools and claims. Built from #2054. The text of §4
above is left as accepted, and this note amends it. Where it disagrees with it, this note holds.

1. **§4, the Kubernetes provider.** "A Kubernetes provider on kubernetes-sigs/agent-sandbox is
   the example extension that proves the seam (RR-36)" now reads: purlis's own **cluster
   operator** runs runners in a Kubernetes cluster. It is a controller a team installs in its
   cluster, not an extension on the desktop, and #906 is superseded by it. The provider seam
   stays for every other backend.
2. **Its objects are purlis's own.** A team defines a `Pool` (cluster-wide) and a person makes a
   `Claim` (in a namespace), in `runners.purlis.dev/v1alpha1`. The cluster operator turns them
   into the objects of one pinned, tested upstream agent-sandbox release whose definitions are
   vendored, and never re-implements what upstream does. Upstream's controller writes its own claim's
   status, which is why purlis's Claim is an object of its own.
3. **Who may use a Pool** is the cluster's own access control: a Claim is admitted only for
   someone the cluster lets `use` the Pool it names, checked by an admission policy as that
   person when the Claim is made.
4. **What the cluster operator may do.** It holds no right to `exec` or `attach` to a pod and
   reads no Secret. docs/cluster-operator.md lists its permissions, and its tests hold them.
