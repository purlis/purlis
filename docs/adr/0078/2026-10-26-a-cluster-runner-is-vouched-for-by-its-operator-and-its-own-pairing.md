# Amended 2026-10-26: a cluster runner is vouched for by its operator and its own pairing

**Built by #2056** on spec #2041's identity decision: *"the runner makes its link key at start;
the operator signs {claim, pod, link key, requester} with a key the desktop pinned when the pool
was added; the desktop also pairs over its own exec, and both must agree; a changed key is
refused and asked about."* It adds to §3 and §8 for runners of a cluster pool, and to the
amendment of 2026-10-17 on a Docker pool's runner (items 3 and 5); it changes neither for a
Docker pool. *Decided in implementation, ruled by the person 2026-10-26* (D-2056-1 to D-2056-8 in its pull
request).

1. **A cluster runner makes its link key as its pod starts, not at its pairing.** The pod's first
   step, `purlis bridge --make-key <file> --publish <file>`, runs the pool's image before
   anything else in the pod. It writes the key's private half to a file it makes (`0600`, never
   through a link) in a folder in memory that only the pod's two containers mount, and the
   public half, as 64 lower-case hex, as its termination message, which the cluster shows in the
   pod's status. Nothing else in the pod runs while it does, so nothing else can publish there.
2. **The operator vouches for it.** It follows the claim to the runner's pod by uid (the
   upstream claim, the sandbox it controls, the pod that sandbox controls), holds the pod to its
   own template for the pool in every field but those the cluster sets, refuses a template
   another field manager changed, and signs, with an Ed25519 key of its own, the statement `purlis runner link key v1` naming the
   claim's namespace, name and uid, the pod's name and uid, the key, and the requester the
   claim was stamped with at admission. The statement is plain text, one `name: value` line
   each in a fixed order, so both sides sign and read exactly the same bytes
   (`fixtures/runner-link-key/`). It is written on the Claim's status with its signature.
3. **The desktop pins the operator's key when it adds the pool**, from the Pool's status, and
   refuses a statement that key did not sign, or one about another claim or requester than the
   one it made as itself (`kubectl auth whoami`).
4. **The desktop also pairs over its own `kubectl exec`**, into the pod the statement names,
   with `purlis bridge --pair --key-made-at-start <file>`: the runner mints its device id there,
   as item 5 of 2026-10-17 says, pairs with the key its pod made instead of making one, keeps it
   in the session host's own folder and removes the pod's copy. The two must agree: a runner
   answering another key than the one vouched for is refused as a changed key, which is how a
   connector that reached another runner is found. So is a statement that, before any later
   link, names another pod or key than the ones paired with; the operator takes a statement
   away once it no longer holds, looking at each vouched claim again every minute.
5. **The connector's own login no longer vouches alone.** §3 let the connector's own
   authentication vouch for the one pairing exchange. For a cluster runner that is the
   operator's statement and the exec answer together: whatever carries the exec stream, or
   points `kubectl` at another pod, cannot substitute a runner without holding the operator's
   signing key or the runner's own link key. Cluster admins and node owners can read both, as
   they can read everything in the cluster (`docs/cluster-operator.md`).
