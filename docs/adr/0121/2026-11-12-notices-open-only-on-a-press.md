# Amended 2026-11-12: Notices opens on a press, never on an arrival

The person reported, from the live app (0.5.2-dev.2190), that "the Notices view always opens
automatically when a new notice comes". This interrupted them, and only a Blocking ask may.

**What opened it.** A Notice of an `ANSWERS` family (`app/src/Notice.tsx`), arriving in the
project's list, brought Notices on screen (`onNoticeAnswer`, ADR 0038 as amended 2026-10-11,
D-1695-3). D-1695-3 meant each of them to answer what the person just did, and two did not:

- **A save's record** (`session-saved`) lands once a chat has written it. That is minutes after a
  Smart close, or with no press at all: a task that reported closes with the chat that asked for
  it, and the core smart-closes it on its own (`handoff::close_reported`). With a fleet of
  dispatched tasks, each one closing opened Notices.
- **The summary of a time away** (`away-summary`) arrives on every return to the window after
  five minutes away when anything happened meanwhile. With agents running, that is nearly every
  return.

Neither was in a person's build alone: both were on `main`.

**Decided.** These two are listed in Notices and move nothing. `ANSWERS` keeps the Notices that
answer a press at once: a refusal (`window-trouble`) and the Undo of a few seconds
(`memory-deleted`, `memory-moved`, `pin-forgotten`). This amends D-1695-3. Each of the two still
goes at once on its Dismiss, with no Undo, as before. A chat's pane band of notices opens only by
the person's choice (minimised or expanded, #4754) and was never opened by an arrival.

The window tests `SmartClose.test.tsx` (a task's record landing with Memory open) and
`AwaySummary.window.test.tsx` (coming back) hold this.
