# Amended 2026-11-09: a runner's claim says from its birth whether its Pool named a key

*Decided in implementation, taken as the recommended option under the operator's standing
instruction to build recommended options.* It adds to the amendment of 2026-10-31 *the operator
mints for a vouched runner* and to the ruling there that only a runner made with
`PURLIS_MINTING_POOL` mints, and keeps everything else in them.

1. **The operator says it as it makes a runner's upstream claim.** A SandboxClaim the operator
   makes for a Claim on a Pool that names an App's key carries
   `runners.purlis.dev/born-minting: "true"`; one made for a Pool that names none carries
   nothing. Every later apply carries what the claim was made with, whatever the Pool says since.
2. **Nobody changes it after.** The policy on what the operator writes,
   `purlis-operator-writes-only-a-runners-shape`, lets it add the annotation, as `"true"`, only on
   CREATE, and its existing rule on a claim's labels and annotations refuses any later addition,
   change or removal. Upstream's controller and a cluster's admins may write upstream's claims
   too, so `purlis-forge` holds the annotation for every writer: only the operator's service
   account adds it, on CREATE, and no UPDATE changes it.
3. **The operator mints only for a runner whose upstream claim says so.** It reads the claim by
   the Claim's name in the runner namespace the statement names, and refuses one that is gone,
   names another Claim, or does not say `"true"`, telling the runner its claim was made before its
   Pool named a key and is to be made again.
4. **Why.** A kept claim's volume lives as long as its claim. A Pool that goes from no key to a
   key keeps that volume, and a Sandbox made again from the current template is marked and
   vouched for while its home is from the time no key was named, when a chat could start outside
   the sandbox. A pod's mark says how its pod was made; only the claim says how old its home is.
5. **Upgrading.** An upstream claim made before this version says nothing, so a runner made
   before it is refused tokens until its Claim is made again. A fresh runner's upstream claim is
   made anew once it has gone idle and runs again.
