# Amended 2026-11-10: the operator acts only under the policies it ships

*Decided in implementation, taken as the recommended option under the operator's standing
instruction to build recommended options.* It changes item 1 of the amendment of 2026-11-01
*a policy's time in force, and a policy on what the operator writes*, adds to its item 2, and
keeps everything else in it.

1. **The operator reads the policies' content, and watches them.** It holds `get`, `list` and
   `watch` on `purlis-claims-use-their-pool`, `purlis-runner-namespaces` and `purlis-forge`, by
   name, as it already did on their bindings. It reads their creation times, as before, and now
   their rules too; a change to the policy on Claims wakes every Claim, as a change to its binding
   does.
2. **A policy is in force only while it is the one the operator ships.** Its validations,
   variables, match conditions, what it matches and its failure policy are compared with
   `deploy/policy.yaml` as compiled into the operator, with the operator's own namespace where the
   install names the release's, and with what the API server fills in when a field is left
   unnamed taken out. Any other content puts nothing in force: Claims say `PolicyNotInForce`, and
   go on when the policy is installed again, at its creation time as before. The policy's
   `generation` is not asked: an upgrade changes the policies in place, together with the
   operator that ships them, and Claims made before it stay under them.
3. **Upstream's objects in a runner namespace are made by the operator alone.** The policy on
   Claims refuses an upstream `SandboxClaim`, `SandboxTemplate` or `SandboxWarmPool` made in a
   namespace named `purlis-runners-*` by anyone but the operator's service account, so what the
   operator finds there under its own names, controlled by the namespace, is what it made.
4. **What it applies in a runner namespace says what it wrote.** Where a quota or network policy
   the operator applied server-side still differs from what it wrote, a field another manager added
   having survived the apply, the operator writes the whole spec over it with a JSON patch, only at
   the version applied. A quota's `scopes` and `scopeSelector` count in that comparison.
5. **Why.** The policies are what the operator's decisions rest on: who a Claim is for, who may use
   a Pool, what a runner namespace holds. Holding the cluster's copy to the operator's own makes
   "in force" mean the rules the operator was built against, and keeps the decision on the
   operator's side of the install.
