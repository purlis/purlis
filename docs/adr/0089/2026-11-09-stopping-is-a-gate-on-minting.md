# Amended 2026-11-09: stopping is a gate on minting

*Decided in implementation, taken as the recommended option under the operator's standing
instruction to build recommended options.* It adds to the amendment of 2026-10-31 *the operator
mints for a vouched runner*, and keeps everything else in it.

1. **A stopped Claim is run again by its requester alone.** A stopped Claim's runner has no key
   vouched for and gets no token, so setting its `spec.stopped` from true to false is held at
   admission, in the policy on Claims, `purlis-claims-use-their-pool`, which now sees UPDATE of a
   Claim too: only the user its `spec.requester` names may do it. Someone else who may patch the
   Claim may still stop it. Any other update is not asked, so the operator's finalizer never
   depends on this rule.
2. **A stopped Pool that mints is run again only with `use` on its key.** Setting a Pool's
   `spec.stopped` from true to false, where the Pool says `forge.mintInCluster: true`, takes `use`
   on the key it names (`forgekeys`), asked of the cluster's authorizer as policy `purlis-forge`
   already asks it for writing `spec.forge`. A Pool that does not mint asks nothing more.
3. **Why.** Before, the only way to take a key away from a Pool's writers, short of deleting the
   Pool, was a change they could undo. With the stop held, an admin stops the Pool and revokes
   `use` on the key, and no writer without that right runs it again. The requester holds the
   Claim's stop for the same reason: a peer who may patch claims in the namespace cannot undo it.
4. **An admin who must run another person's Claim again acts as its requester**
   (`kubectl --as`), as for its repos.
