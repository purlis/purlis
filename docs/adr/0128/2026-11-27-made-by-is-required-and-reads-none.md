# Amendment 2026-11-27: `made_by` is required and reads "none"

Amends [ADR 0128](../0128-your-sign-in-brokered-one-approach-for-every-forge.md) as the forge
operations' amendment of the same day left it (#7776, its §2: `made_by` optional and never set),
with the person's ruling **Q339.2** of 2026-11-27 (#7748).

- Every forge operation's context (`ForgeCtx`) carries `owner`, whose the branch or object acted
  on is, in the words of [ADR 0125's amendment of the same day](../0125/2026-11-27-the-host-holds-who-owns-what-a-chat-acts-on.md):
  `"self"`, `"task"`, `"asker"`, `"other"` or `"none"`.
- `made_by`, who made the forge object an operation names (a comment, an issue), is **required**,
  not optional as first written. It reads `"none"` until M81 (#7793) keeps a record of the objects
  a chat made, so a rule reads it with no `has`, and a request without it does not fit the schema.
- A rule that holds a chat to its own objects writes `context.made_by != "self"`: a forbid on
  `== "other"` does not fire on `"none"`.
