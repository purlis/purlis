# Amended 2026-11-09: a Pool's forge writer stays answerable at each mint

*Decided in implementation, taken as the recommended option under the operator's standing
instruction to build recommended options.* It adds to the amendment of 2026-10-31 *the operator
mints for a vouched runner*, and keeps everything else in it.

1. **A Pool that names a key says who wrote its forge.** The annotation
   `runners.purlis.dev/forge-writer` holds the name the cluster signed that person in as. The
   policy `purlis-forge` holds it: on CREATE or UPDATE of a Pool, whenever `spec.forge` or the
   annotation changes, the annotation must equal `request.userInfo.username` of that write. A
   Pool with no `spec.forge` needs none, and taking `spec.forge` off asks nothing more.
2. **The operator asks of that writer before each mint.** Beside its question whether the
   Claim's requester may still `use` the Pool, it asks the cluster's authorizer whether the
   writer may still `use` the key: `use` on `forgekeys` in `runners.purlis.dev`, named for the
   Pool's `appKeySecretRef`, exactly as the policy asks it of whoever writes the forge. A Pool
   that names no writer, or whose writer may not, mints nothing, and the runner is told to have
   the Pool's forge written again by someone who may use the key.
3. **Why.** `use` on a key was asked only as a Pool was written, so revoking it stopped further
   writes and left a Pool already written minting. With the writer recorded and asked again,
   revoking `use` on a key stops minting for every Pool its holder wrote, at the next mint.
4. **By name alone.** The writer is asked of by name, as the requester is, since the Pool records
   no groups: a right on the key held only through a group is not seen, and refuses rather than
   allows. A cluster admin grants `use` on a key to the person by name.
5. **Upgrading.** A Pool written before this carries no writer and mints nothing until someone
   who may `use` its key names themselves on it (`kubectl annotate`), which the policy lets
   them do without writing the forge again.
