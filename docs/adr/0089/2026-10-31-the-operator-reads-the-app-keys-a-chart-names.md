# Amended 2026-10-31: the operator reads the GitHub App keys its chart names

**Built by #4204** on the person's rulings of 2026-10-30 on #3807 (Q2, Q4, Q5, Q6). It amends item
4 of the amendment of 2026-10-14 (*"reads no Secret"*), item 2 of the amendment of 2026-10-26 (*"It
reads no Secret through the API"*) and the sentences repeating them in the amendments of 2026-10-29,
and keeps everything else in them.

1. **The operator may `get` the Secrets the chart names, and no others.** A Role
   `purlis-forge-key-reader`, in the operator's own namespace, grants `get` on `secrets`, limited by
   `resourceNames` to the chart's `forge.keySecretNames`. It never lists, watches or writes a
   Secret, and the ClusterRole still holds no Secret right.
2. **With no name set there is no Role.** An empty `resourceNames` would mean every Secret, so the
   chart renders neither the Role nor its binding, and the default install, the plain manifests
   included, still reads no Secret through the API.
3. **Its signing key stays out of this.** The kubelet still mounts it, and neither the chart's
   values nor a Pool may name it. Whoever reads it vouches for any runner.
4. **A Pool opts in; a Secret is read only for one that does.** `Pool.spec.forge` holds
   `appKeySecretRef` (`{name}`, in the operator's own namespace), `repos` (a required allow-list of
   `owner/name` or `owner/*`) and `mintInCluster`. Being named by the chart is not enough to be read
   for a Pool, and being named by a Pool is not enough to be readable: both must say so. The Secret's
   data keys are `app` (the App's client id) and `key` (the PEM).
5. **A Claim names exact repos, and only its requester changes them.** `Claim.spec.forge.repos`
   holds `owner/name` alone, since a wildcard there would make the intersection with a Pool's
   `owner/*` every repo of the owner. The policy on Claims now also sees UPDATE, for this one rule.
6. **A persona tool line naming another App is refused by the operator**, which alone holds the
   Secret, when a runner asks it to mint (#4206).

This amendment mints nothing. #4206 adds the minting and #4207 the chat that uses it.
