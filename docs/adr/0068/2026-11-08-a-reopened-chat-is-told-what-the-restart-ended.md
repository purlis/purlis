# Amended (2026-11-08): a reopened chat is told what the restart ended (#5743)

Records spec [#5743](https://github.com/purlis/purlis/issues/5743), ruled by the person on
2026-11-08 in a grilling over the research matrix of #5716
(`docs/research/restart-own-work-cases.md`). Every recommendation was accepted. The tracer
[#5747](https://github.com/purlis/purlis/issues/5747) builds the first part. The other rules below
are decisions made ahead of the behaviour, and each ships with its sibling ticket under #5743.

A restart that ends a chat's program also ends the chat's **own work**: its background shells,
monitors, subagents, workflows, crons and wakeups. The resumed harness never hears about it. A chat
that was waiting to be woken by that work would wait for ever.

- **Own work is recorded on disk each time it changes**, not only at a quit, because a session host
  crash writes nothing as it goes. The record is kept per chat in the record of open chats
  (`app/reopen.json`, `chats[].own_work`; Clone state, ADR 0069; not a format change). Each turn's
  end of the chat's own harness fills it. The 60-minute lapse still hands the chat to the person,
  as #3878 and #4647 set it, but it no longer clears the record.
- **A reopened chat is given one wake line.** It is purlis's own fixed message, typed in when the
  chat comes back on its conversation. It says what the restart ended, and the chat decides what
  to start again. **purlis never re-runs a chat's work.** A dispatched task that still owes its
  report gets the carry-on (ADR 0090's restart, #1513) inside the same line, never as a second
  message. A chat the session host kept running (#3900) gets none, because nothing of it ended.
  Neither does a chat started with no conversation, nor a chat with nothing to tell.
- **The wake line grows** under #5743, to cover:
  - each piece's last output and where the whole output is;
  - the crons and wakeups that were lost, including any that came due while purlis was down;
  - a turn that was cut mid-way;
  - the news of the chat's tasks.

  It is handed out at a launch and on every other reopen.
- **This ADR's update row is read as the code behaves.** An update ends chats as a quit does
  until #665 lands, and its chats get wake lines as a quit's do.
- **The person is warned, and nothing is blocked.** The quit warning and the update dialog list
  the chats waiting on their own work and what will end. The host-crash notice no longer says
  nothing was lost when something was. Restarts made "now" go ahead, and every other restart
  waits while the chat waits on its own work, as the window's restarts already do.
