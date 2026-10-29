# Amended 2026-10-26: the operator reads a runner's pod and signs with a mounted key

**Built by #2056** on item 4 of the amendment of 2026-10-14 (*"It holds no right to `exec` or
`attach` to a pod and reads no Secret"*), which it keeps, and spec #2041's *"the operator ...
vouches for every runner's identity"*. *Decided in implementation, ruled by the person 2026-10-26* (D-2056-2,
D-2056-3 in its pull request). The key is rolled, and a statement names its cluster, since
the amendment of 2026-10-29 (#3305): [the operator's signing key is rolled](2026-10-29-the-operators-signing-key-is-rolled-and-a-statement-names-its-cluster.md).

1. **The operator reads a runner's sandbox and pod, by name, and nothing more of them.**
   Vouching for a runner's link key needs the key its pod published in its status, and the
   chain from the claim to that pod by uid (ADR 0078, amended 2026-10-26), so its ClusterRole
   gains `get` on `pods` and on upstream's `sandboxes`, and `patch` on `pools/status` to name its
   signing key on every Pool. It never lists, watches, makes, changes or deletes a pod, and
   still holds no `exec`, `attach`, port-forward or Secret right.
2. **It reads no Secret through the API; the kubelet mounts its one signing key.** The person installing the operator
   makes the key once (`openssl genpkey -algorithm ed25519`, piped, never written to a file)
   and keeps it as the Secret
   `purlis-cluster-operator-signing-key` in its namespace, which the kubelet mounts in the
   operator's pod, read-only and for its group alone. Without it the operator does not start:
   it vouches for no runner unsigned.
