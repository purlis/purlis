# Amended 2026-10-26: the operator reads a runner's pod and signs with a mounted key

**Built by #2056** on item 4 of the amendment of 2026-10-14 (*"It holds no right to `exec` or
`attach` to a pod and reads no Secret"*), which it keeps, and spec #2041's *"the operator ...
vouches for every runner's identity"*. *Decided in implementation, to be confirmed* (D-2056-2,
D-2056-3 in its pull request).

1. **The operator reads a runner's pod, by name, and nothing more of pods.** Vouching for a
   runner's link key needs the key its pod published in its status (ADR 0078, amended
   2026-10-26), so its ClusterRole gains `get` on `pods`, and `patch` on `pools/status` to name
   its signing key on every Pool. It never lists, watches, makes, changes or deletes a pod, and
   still holds no `exec`, `attach`, port-forward or Secret right.
2. **Its signing key is a Secret it holds no right to read.** The person installing the operator
   makes the key once (`openssl genpkey -algorithm ed25519`) and keeps it as the Secret
   `purlis-cluster-operator-signing-key` in its namespace, which the kubelet mounts in the
   operator's pod, read-only and for its group alone. Without it the operator does not start:
   it vouches for no runner unsigned.
