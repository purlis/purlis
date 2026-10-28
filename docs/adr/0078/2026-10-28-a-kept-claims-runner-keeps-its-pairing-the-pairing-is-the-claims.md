# Amended 2026-10-28: a kept claim's runner keeps its pairing; the pairing is the claim's

**Built by #2057** on the person's ruling of 2026-10-28 on #2057: *"A kept claim keeps its
pairing when its pod restarts. Pairing is tied to the claim, not to the pod uid, so D-2056-7 and
the signed statement change to name the claim."* It amends the amendment of 2026-10-26 (a
cluster runner is vouched for by its operator and its own pairing), items 2 and 4, and D-2056-7
and D-2056-8 of #2056's pull request (#3293), for runners of a cluster pool only. A Docker pool's
runner is unchanged. *Ruled by the person 2026-10-28* (the names and the pairing); *decided in
implementation* (D-2057-1 to D-2057-19 in its pull request).

1. **A claim is kept or fresh.** A kept claim's runner has a volume of its own,
   `purlis-kept-<claim>`, and its home is on it (`HOME=/var/lib/purlis/home`), so its checkout,
   conversation, profile sign-in, kept browser logins and its pairing (device id, link key and
   peers file) outlive its pod. A fresh claim's runner keeps nothing past its pod.
2. **The pairing is the claim's, not the pod's** (amends D-2056-7). The 2026-10-26 amendment's
   item 4 and D-2056-7 had the pairing take the key the pod made as it started, keep it in the
   session host's own folder and remove the pod's copy: a new pod was a new runner, to be
   claimed again. The session host's own folder is now in the runner's home, so on a kept
   claim's volume it outlives the pod. When the pod is made again, its first step, `purlis
   bridge --make-key <file> --publish <file> --home <dir>`, finds the pairing there and
   publishes the key the runner was paired with, handing over no new one. The desktop links to
   the same runner without pairing again. A home that holds no pairing gets a new key, as
   before, and a runner is still paired once, from a home that came with no device id.
3. **The statement names the claim, and the pod by name only** (amends item 2 and D-2056-8).
   The statement is now `purlis runner link key v2`: the claim's namespace, name and uid, the
   pod's name, the key and the requester. The pod's uid, which changes when a pod is made
   again, is no longer in it. Before every link, the desktop refuses a statement that names
   another pod or key than the ones it paired with, as a changed key. A kept claim's pod is made
   again under the claim's name, with the key it keeps, so the statement says the same. A fresh
   claim's new pod makes a new key, which is refused as before. A `v1` statement is not read.
   The statement also names when it stops holding (`until`, ten minutes after it is signed):
   the operator signs it again in its last five minutes while it still vouches, and the
   desktop refuses one past its time, or one that claims to hold more than an hour.
4. **The operator still holds every pod to its template, now one per claim's kind.** A kept
   claim's pod must carry exactly its own volume, `purlis-kept-<claim>`, in the place of the
   template's folder, as upstream puts it there, and its sandbox must have the claim's name. Any
   other volume refuses the pod, as any other difference from the template does.
5. **Who can take a kept runner's identity** is whoever can read its volume or make a pod that
   mounts it in its claim's namespace, as whoever may exec into its pod could before, and
   whoever holds the node or the storage behind it. The namespace is the boundary, as the
   2026-10-26 amendment says (`docs/cluster-operator.md`).
