# Amendment, 2026-11-17: every rule's `@id` carries its family's prefix, and the catch-all is `policy-everything-else` (Q326)

The decision says every rule carries an `@id`, and Q316 point 12 that an `@id` is the family
prefix plus a kebab description. The prototype's sample projects wrote permits without one
(`allow-known-hosts`), and the catch-all, `everything-else`, spans every family and so had no
family to take a prefix from. Q326 (the person, 2026-11-17, while slice 01 of M75 was built,
#7009) rules:

- **The family prefix is required on every rule's `@id`, permits included**: the family's
  `[policy]` key with `.` as `-`, then a hyphen (`network-allow-known-hosts`,
  `guards-git-infra-deployer-push-main`). A rule over the actions of several families takes the
  prefix of any one of them.
- **The catch-all is `policy-everything-else`.** The prefix `policy-` is reserved for the shipped
  default's structural rules, and a rule in any other file that takes it is refused at load.
- So deny-by-default is `[policy.rules] "policy-everything-else" = "off"`, and its refusal reads
  `policy-everything-else (off)`, where the decision wrote `everything-else`.
