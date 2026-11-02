# Amended (2026-11-02): every chat keeps the message its turn ended on, in memory

Implements #4550, on the operator's filing of that issue.

Until now the message a `Stop` hook hands over was kept for a task alone, so a person's own chat
waiting on a reply had nothing to show in its Inbox card. **The message is now kept for every
chat**, on the same terms, and it has one source, `Ledger::last_said`, which spec #4551 reads too:

- It is masked and bounded as a report's text is (`handback::last_message_summary`) before it is
  kept, and the line it arrives on is still never spooled.
- It is held in memory only, the latest message alone, until the chat's next turn begins or the
  chat closes. After a relaunch the app knows no message, and the card says so.
- It is the Inbox's to draw. A notification still says an ask's kind, never its words, and nothing
  of it reaches the audit trail or telemetry (ADR 0075).

What a task's chat that asked may read, the last three messages and twenty commands, stays a
task's alone. A chat that is no task keeps its latest message and no history.
