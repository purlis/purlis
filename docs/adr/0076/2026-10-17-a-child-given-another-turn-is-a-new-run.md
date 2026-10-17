# Amendment, 2026-10-17: a child given another turn is a new run

Decided in implementation (#2455, PR #2479); acknowledged by the operator on 2026-10-18.

§6 gives a child run three states and one way in: the first hook carrying a **new** `agent_id`.
It said nothing of a child heard again after its own `SubagentStop`. Codex runs `SubagentStop`
at the end of each turn of a child, not once for its life, and a parent can give a finished
child another turn (Codex's `send_input`, a resumed Claude Code sub-agent) whose hooks carry the
same `agent_id`. The board then showed the child `completed` while it worked.

- **A known `agent_id` heard doing work after its `SubagentStop` starts a new run of that
  agent**, `working`. Doing work is a tool call, a prompt or an ask under that `agent_id`. Its
  next `SubagentStop` ends that run `completed | child-ended`, as §6's row says. Once
  `Run::child` is wired to the event log, the new run begins with cause `child-seen`, as a
  first sighting does.
- **A run still ends exactly once.** The run that completed stays completed; nothing moves it
  from `completed` back to `working`. The board's row for an `agent_id` shows that agent's
  latest run.
- **Any other hook of a finished child moves nothing**: its `Stop`, its session's start or end.
  Only work starts a new run, so a late hook cannot show a finished child working with no
  `SubagentStop` to follow.
- **A child that ended with its parent starts no new run.** Its chat has ended, and an ended
  chat passes no hook of a child on.
