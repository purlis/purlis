# Amended (2026-11-02): a chat keeps a few turn-end messages, through a background turn

Implements #4563, the waiting strip of spec #4551, on the person's review of its prototype (#4561).

The amendment of the same day kept the latest message a chat's turn ended on, until its next turn
began. The waiting strip needs two more things from that one source, `Ledger`:

- **A few, not one.** Each chat keeps its last four turn-end messages (`SAID_KEPT`), so a question
  a later turn pushed aside can be read back: `Ledger::last_said` is the last, and
  `Ledger::said_before` the ones before it, oldest first. The registry hands them on a reply's ask
  as `last_message` and `earlier_messages`.
- **Kept through a background turn** (#4562). A turn the person did not begin is no reply, so it
  clears nothing: the turn their reply begins, and the chat's close, clear them all.

**A long message keeps its head and its tail.** Past a report's 4 KiB bound, the text is cut in its
middle, not at its end, so a question on the last line survives. A report purlis makes from a
task's last message is cut the same way.

Unchanged: masked and bounded before it is kept, in memory only (after a relaunch the strip and the
card say there is no message, until the next turn ends), and never in a notification, the audit
trail or telemetry (ADR 0075). A real ask (a permission, Claude Code's question tool) is shown in
its own words, held with the ask, never by these messages.
