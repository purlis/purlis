# Amended 2026-10-29: a turn may end waiting on the chat's own work (#3878)

**Supersedes D-1644-1** (#1644). That decision left a chat's own scheduled wake-up, its
monitors and its background commands unread, so a turn that ended on them was "needs you". A
chat waiting on its own background work then showed ✋ and sent a notification though nothing
was asked of the person, and the chat that dispatched it read it as idle. D-1644-1 said this
needed a state of its own. The operator ruled on that state on 2026-10-29.

**The neutral model has a new turn, `WaitsOnItsOwnWork`.** It carries what the chat waits on
(`purlis_core::ownwork::OwnWork`): how many helpers, workflows, monitors and background shells
are in flight, when its next wake-up of its own is due, and when the wait lapses. It replaces
`AwaitsItsHelpers` (#1626), which counted only helpers and showed the chat as working.
Claude Code is the only adapter that fills it, from its `Stop` input's `background_tasks` and
`session_crons`. Every other harness hands each end of turn to the person, as before.

**What counts:**

- A helper (`subagent`), a `workflow` or a `monitor` counts for as long as it is in flight, with
  no limit, because each one ends by itself or wakes the chat.
- A one-shot wake-up counts until it is due, plus a few minutes for the harness to fire it.
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
- at once when it asks: a permission, a question, or a sandbox block;
- at its limit, on the app's clock. The board reads no clock, so the app ends the wait at the
  time the wait carries (`Board::own_work_lapsed`, a move of its own), whether or not the chat
  ever wakes. The turn's end is then the person's, or is held for the chat's tasks as any end
  of turn is (#1491).
