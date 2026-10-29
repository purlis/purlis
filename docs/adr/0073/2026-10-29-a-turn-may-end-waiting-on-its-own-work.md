# Amended 2026-10-29: a turn may end waiting on the chat's own work (#3878)

**Supersedes D-1644-1** (#1644). That decision left a chat's own scheduled wake-up, its
monitors and its background commands unread, so a turn that ended on them was "needs you". A
chat waiting on its own background work then showed ✋ and sent a notification though nothing
was asked of the person, and the chat that dispatched it read it as idle. D-1644-1 said this
needed a state of its own. The operator ruled on that state on 2026-10-29.

**The neutral model has a new turn, `WaitsOnItsOwnWork`.** It carries what the chat waits on
(`purlis_core::ownwork::Said`): how many helpers, workflows, monitors and background shells
are in flight, and in how many seconds from the turn's end its next wake-up is due and the
wait lapses. It replaces `AwaitsItsHelpers` (#1626), which counted only helpers and showed the
chat as working. The rules for what counts are neutral (`purlis_core::ownwork::said`). Claude
Code's adapter (`harness::claude::ownwork`) is the only one that fills it, from its `Stop`
input's `background_tasks` and `session_crons`. Every other harness hands each end of turn to
the person, as before.

**Durations, never a time of day, cross from the chat to the app.** Claude Code fires a cron
on the wall clock of the machine the chat runs on, which may keep another clock or time zone
than the app (a Docker or Kubernetes runner). The hook reads each schedule there and sends the
seconds until it is due. The app stamps them on its own clock as it applies the turn's end
(`Waits::now`, carried in the move so a replay stamps the same). A wait with a limit and no
clock said is not held: the end is the person's.

**What counts:**

- A helper (`subagent`), a `workflow` or a `monitor` counts for as long as it is in flight, with
  no limit, because each one ends by itself or wakes the chat.
- A one-shot wake-up counts until it is due, plus a few minutes for the harness to fire it. One
  due in the minute the turn ends in, or overdue (it matched in the last day and is still
  listed), is due now, and lapses those few minutes later.
- A background `shell` counts for **60 minutes** after the turn ends (`OWN_WORK_LIMIT`), because
  a dev server or a log tail may run for as long as the chat does.
- A recurring cron counts only when its next run is within those 60 minutes.

A payload this cannot read counts as nothing, which leaves the end of the turn with the person.

**The board shows the chat as waiting.** Its state is `waiting`, it is not in the needs-you
queue, no notification is sent, and its idle nudge raises nothing. The window says what it waits
on, in the working colour with the hourglass that a chat waiting on its tasks already wears:
`waiting on 2 monitors`, or `waiting on a wake-up at 14:05`. A chat that dispatched it reads
`waiting on its own work` in its task list, its waits and its roster. It never reads idle.

**The wait ends:**

- when the chat's next turn begins (its work woke it);
- at once when it asks: a permission, a question, or a sandbox block. A block's Notice raised
  in the turn makes that turn's end the person's; one raised after it, by a monitor or a
  background shell, ends the wait as it goes up (`Board::blocked_while_waiting`, a move of its
  own);
- at its limit, on the app's clock. The board reads no clock, so the app ends the wait at the
  time the wait carries (`Board::own_work_lapsed`, a move of its own), whether or not the chat
  ever wakes. The turn's end is then the person's, or is held for the chat's tasks as any end
  of turn is (#1491). Where the app cannot start the timer, the wait ends at once: it fails
  toward telling the person.
