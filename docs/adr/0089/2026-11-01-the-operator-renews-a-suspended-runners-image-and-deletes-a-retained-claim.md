# Amended 2026-11-01: the operator renews a suspended runner's image and deletes a retained Claim

**Ruled by the person on 2026-11-01**, on #3982 and #3981, also for #3739; built by them. It amends
the amendment of 2026-10-28 (*"the operator suspends a stopped runner"*), item 3, and gives the
operator the deletion #3739 asked to be ruled.

1. **The operator may also change a Sandbox's `spec.podTemplate`, only on a Sandbox already
   `Suspended` in a `purlis-runners-*` namespace.** No other field or namespace. Upstream copies
   the template once and never updates it, and remaking its upstream claim would take a kept
   Claim's volume, whose home is tied to that claim's uid. The sequence is suspend, patch, run;
   the operator changes only images, and vouching still checks the pod against the pool's
   template. The policy `purlis-operator-changes-only-a-sandboxs-mode` holds this.
2. **The operator gains `delete` on `runners.purlis.dev/claims`, narrowed by the policy
   `purlis-operator-deletes-only-a-retained-claim`** to a kept Claim, not stopped, carrying the
   operator's own `RetentionEnded` condition. Retention counts only idle suspension, so `stopped`
   still never destroys.
3. **A chat is never ended for a maximum age** (*decided in implementation, D-3982-1*): §9's
   overnight work stands. A runner past it waits until it is next idle, and its Claim says so.
