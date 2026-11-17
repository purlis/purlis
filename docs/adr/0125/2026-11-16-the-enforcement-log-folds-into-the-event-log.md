# Amendment, 2026-11-16: the enforcement log folds into the event log (Q313)

Decision 1's `log` mode wrote one quiet line to the chat's enforcement log,
`app/enforcement/<chat>.jsonl`, and the Consequences said the app alone writes it.
[ADR 0126](../0126-one-project-policy-governs-every-chat-decision-cedar.md) supersedes that store
(Q313 point 3):

- **Allow, ask and deny all go into the existing event log; there is no store of its own.** Each
  event gains the deciding rule ids, the level, the family, the outcome and the outcome after the
  mode: an allow on the tool event, an ask on `ask.raised` and `ask.answered`, a deny on
  `stop.block`.
- **A family in `log` writes a would-have-denied line** there, in place of the enforcement log's
  line.
- As before, no line holds a command, a path or a URL, and only the app writes the log.

`app/enforcement/` is retired, and `docs/plane-format.md` drops it with the slice that moves the
lines.
