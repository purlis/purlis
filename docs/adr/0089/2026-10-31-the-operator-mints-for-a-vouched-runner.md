# Amended 2026-10-31: the operator mints a GitHub App token for a vouched runner

**Built by #4206** on the person's rulings of 2026-10-30 on #3807 (Q2, Q3) and of 2026-10-31. It adds
to the amendment of 2026-10-31 on the App keys, and to the operator's rights in the amendments of
2026-10-14 and 2026-10-29, and keeps everything else in them.

1. **The operator serves a mint port.** TLS 1.3 on 8443, its certificate self-signed by the signing
   key, so a client holds it to the key on every Pool's status, not to an authority. A Service and a
   way in by NetworkPolicy exist only where the chart names a key.
2. **A runner proves its link key with a challenge.** A link key is X25519 and cannot sign, so the
   operator answers a one-use X25519 challenge, and the runner answers with a MAC keyed by their
   agreement over what it asks and the operator key it holds the port to.
3. **Writing a Pool's `spec.forge` takes `use` on the key it names** (ruled 2026-10-31): the verb on
   `forgekeys`, by the Secret's name, held at admission. `forgekeys` names a right, as no such
   object exists. On a Pool that mints, changing its image takes the same right, since the image
   holds the link keys the operator vouches for (decided in #4252, stricter than the ruling asks).
4. **The rules on App keys are a policy and binding of their own, `purlis-forge`.** The rule on who
   changes a Claim's repos moves there from the policy on Claims, which again sees only CREATE of a
   Claim. The operator mints only for a Claim and a Pool made once both the policy and its binding
   have settled, so a binding's time means these rules were in force.
5. **The operator asks the cluster before each mint** whether the requester may still `use` the Pool
   (ruled 2026-10-31): a new right to create SubjectAccessReviews. It asks by name alone, since a
   Claim records no groups, so a right held only through a group is refused, and a group's change
   can never leave a stale yes.
6. **The operator makes one NetworkPolicy, `purlis-forge-mint`**, in a runner namespace alone, whose
   one rule is the way out to its own mint port, as the policy on runner namespaces holds. Upstream's
   policy on the runner stays whole; NetworkPolicies add up.
7. **A mint names its repositories and its permissions**, the permissions being what the
   installation holds of a built-in list of repo-level ones that administer nothing, at `write` at most (ruled 2026-10-31: no Pool key for now), and what comes
   back is held to them. A Pool cannot widen that list today.
8. **A Secret is read as text** and parsed by the operator, which logs nothing of a body it cannot
   parse. The key is read for each mint and not kept.

#4207 makes a chat use this.
