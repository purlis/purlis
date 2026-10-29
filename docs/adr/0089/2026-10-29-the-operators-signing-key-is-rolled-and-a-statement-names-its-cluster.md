# Amended 2026-10-29: the operator's signing key is rolled, and a statement names its cluster

**Ruled by the person on 2026-10-29** (E1 to E4 on #3305, accepted as recommended), built by
#3305. It amends item 2 of the amendment of 2026-10-26 (*"the kubelet mounts its one signing
key"*) and item 5 of the amendment of 2026-10-29 on runner namespaces (*"What the cluster
operator may do"*), and keeps everything else in them: the operator still reads no Secret
through the API, and holds no `exec` or `attach` right to a pod.

1. **One Secret, two keys.** `purlis-cluster-operator-signing-key` holds `key.pem`, the key the
   operator signs with, and during a roll `next.pem`, the key it rolls to. The operator's pod
   mounts the whole Secret, not one key of it, so the kubelet refreshes both files when a team
   admin changes them, and the operator reads them again, both from one version of the Secret,
   each time it signs a statement or looks at a Pool: a roll needs no restart.
   A `next.pem` it cannot read stops the operator from starting, and while it runs leaves the
   keys it last read in use.
2. **The Pool's status names the next key, and the current key vouches for it.** During a roll
   every Pool's status names `nextSigningKey`, spelled as `signingKey`, and
   `nextSigningKeyVouch`: a statement, `purlis cluster operator key roll v1`, of the cluster,
   the key it rolls `from`, the key it rolls `to` and an `until` seven days on, signed by the
   current key, and signed again in its last day while the roll lasts. A desktop pinned to the
   current key that reads a word it signed for this cluster, from its pinned key to the Pool's
   next key, holding now and for no more than seven days and an hour, moves its pin to the next
   key and asks no one. A desktop that missed the roll refuses the new key and asks again.
3. **Statements are signed by both keys during a roll.** A Claim's `status.linkKey` carries
   `nextSignature`, the next key's signature over the same statement, beside `signature`. A
   desktop that has moved its pin reads `nextSignature`, so the old key is trusted by no
   desktop that has moved, from the moment it moves.
4. **The cluster is the uid of its `kube-system` namespace.** The operator reads it once as it
   starts, through its `get` on namespaces (which the amendment of 2026-10-29 on runner
   namespaces gave it; this amendment names `kube-system` as a namespace it reads), names it
   on every Pool as `status.cluster`, and writes it first in every statement, as a `cluster:`
   line in v3 of the statement (`purlis runner link key v3`). A desktop pins the cluster with
   the key when it adds the pool, and refuses a statement, a Pool or a key roll's word of any
   other cluster. v2 of the statement is no longer read: the desktop and the operator go
   together, of one release.
5. **A statement's age is its `until`**, ten minutes on, as the amendment of 2026-10-28 made
   it; a Pool has no field of its own for it.
