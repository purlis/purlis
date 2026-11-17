# Amendment, 2026-11-16: `[policy] sandbox` sets the sandbox level (Q315, Q318)

Decision 1 said the sandbox is a level, `off`, `floor` or `full`, which the template sets through
the existing `[sandbox]` mode, so "`[policy]` has no `sandbox` key". [ADR 0126](../0126-one-project-policy-governs-every-chat-decision-cedar.md)
supersedes that (Q315 point 2, Q318 point 9):

- **`[policy] sandbox = off|floor|full`** is the `sandbox` family's one key. The level is set by
  the template or by that key. It replaces `[sandbox] mode`, and `[sandbox]` is gone as a table
  of settings (ADR 0126 decision 6, a clean cut).
- **Under `strict`, `[policy] sandbox` sets the level**, where decision 2's table said "as
  `[sandbox]` says" (Q318 point 9).
- `[sandbox] policy = local|never` becomes a Project lock on the `sandbox` family.

The level stays a level, not a mode: the `sandbox` family has no `off`, `log` or `enforce` of its
own.
