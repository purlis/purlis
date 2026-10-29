# Amended 2026-10-29: each person's runners run in a namespace of their own

**Ruled by the person on 2026-10-29** (option A on #2055), built by #2055. It amends item 4 of
the amendment of 2026-10-14 (*"What the cluster operator may do"*) and keeps everything else in
it and in the amendment of 2026-10-26: the operator holds no `exec` or `attach` right to a pod
itself, reads no Secret, and binds only `purlis-runner-owner`, only in runner namespaces (item 6).

1. **A namespace per person.** For each requester the cluster operator makes the namespace
   `purlis-runners-<hash>`, the first 16 hex digits of the SHA-256 of the requester, and runs
   every runner of that person's Claims there, whichever namespace the Claims are in. The Claim
   stays where it is made. The signed statement names the runner namespace (`runner namespace:`,
   in v2 of the statement), and a desktop refuses one naming any other than the requester's own.
2. **The person alone has a right there.** A RoleBinding in it gives the requester alone the
   cluster role `purlis-runner-owner`: `get`, `list` and `watch` on pods, plus `pods/exec` and
   `pods/log`.
3. **A quota per person.** A ResourceQuota `purlis-person` in it, from the chart's
   `perPerson.quota`: by default 5 claims, plus CPU, memory and storage. A Claim whose runner
   would take its requester over it says `Ready` `False` with the reason `OverQuota`.
4. **A finalizer does the cleanup.** Each Claim the operator acts on carries
   `runners.purlis.dev/runner`, and the operator removes what it made for the Claim in the runner
   namespace before it lets the Claim go, since an owner in another namespace cannot own it.
5. **What the cluster operator may do** gains, for this alone: `create` on namespaces,
   rolebindings and resourcequotas; `get` on namespaces; `get` and `patch` on the one
   rolebinding (`purlis-runner-owner`) and the one quota (`purlis-person`) by name; `bind` on
   the one cluster role `purlis-runner-owner`; `patch` on Claims for the finalizer; and `delete`
   on the three upstream objects it makes. It holds no right to delete a namespace: a runner
   namespace stays, empty, after its person's last Claim. docs/cluster-operator.md lists its
   permissions, and its tests hold them.
6. **A policy holds runner namespaces at admission**, since RBAC cannot say where a role may be
   bound or a namespace made: `purlis-runner-namespaces`, a ValidatingAdmissionPolicy installed
   beside the policy on Claims. Only the operator makes a runner namespace or changes who it is
   for; the operator makes no other namespace, binds only `purlis-runner-owner`, only in a runner
   namespace, for one subject that is not itself, and makes only the quota `purlis-person`, only
   in a runner namespace. The operator acts only in a runner namespace made while that policy was
   in force, as it acts only on Claims made under the policy on Claims.
