# Amended (2026-11-08): a reopened chat is told what the restart ended, and purlis never re-runs it

The person ruled on this on 2026-11-08, in a grilling over the research in
[`docs/research/restart-own-work-cases.md`](../../research/restart-own-work-cases.md). It is
recorded here ahead of the behaviour, which ships with its spec.

**The problem.** A chat whose turn ended with **own work** still in flight (a background shell,
a monitor, a subagent, a workflow, a cron or a wakeup; ADR 0073's 2026-10-29 amendment) loses
that work at every restart that ends its program: a quit, an update, a window crash past the
grace period, a session host crash, and a restart the person makes "now". The harness is
reopened on its conversation, and nothing tells it that the work is gone. A chat that was
waiting to be woken then waits for ever.

**The decision.** When purlis reopens a chat that the restart cut off, it types one **wake
line** into the chat. The line names each piece of own work the restart ended, gives each
piece's last output, and says which turn was cut mid-way. It also carries the chat's
dispatched tasks and any report that arrived or was pending. The chat decides what to start
again. **purlis never re-runs a chat's own work.**

**Alternatives the person turned down.**

- **purlis re-runs the work.** purlis cannot tell whether a shell is safe to run twice, and a
  monitor or a workflow that runs again repeats its side effects. Only the chat knows whether
  the work is still wanted.
- **A mark on the board only.** The chat would still sit idle until the person happened to
  notice the mark.

**What follows from it.**

- **Own work is kept on disk** as it changes, not only at a quit, because a session host crash
  writes nothing. Each piece is recorded when its tool starts it, and the turn's `Stop`
  remains the authority on what is still running. This is a store change under ADR 0069.
- **A chat the host kept running gets no wake line.** Its program never stopped (#3900,
  `left_running`).
- **The wake line is typed when the chat is reopened, never at launch.** It goes in when the
  person or **Reopen all** reopens the chat, or when a hibernated chat wakes. It follows Reopen
  all's pacing and the cap on running chats.
- **Restarts the person makes "now" end the work, and the chat is told.** These are **Restart
  now**, restarting without the sandbox, and containing the project's settings. Every other
  restart still waits while the chat waits on its own work (#3887). The core's
  `restart_chat` now waits too.
- **The 60-minute lapse changes only what the board shows.** After it, a restart no longer
  waits, but the work it ends is still listed in the wake line.
- **purlis ends the whole process group when it ends a chat.** No piece of own work outlives
  the harness, so "ended" in the wake line is always true.
- **An update ends chats exactly as a quit does until #665 lands.** This amends this ADR's
  table, whose "restarts to update" row says that an update keeps every session. That row
  describes #665's goal, not today's behaviour. The wake line covers updates in the meantime.

**Scope.** Only Claude Code's own work is read (ADR 0073's 2026-10-29 amendment). Codex and
opencode need their harness to report background work first, and that is a ticket of its own.
Mac sleep ends nothing, so it needs no wake line.

**The store, as built by the tracer (#5747).** Own work is a per-chat key, `chats[].own_work`,
of the record of open chats (`app/reopen.json`), in that record's tier, Clone state (ADR 0069),
at its format version 1: the key's absence reads as nothing in flight, so it is not a format
change ([`docs/plane-format.md`](../../plane-format.md)). The tracer writes it at each `Stop` of
the chat's own harness, from what that `Stop` said; recording each piece as its tool starts it
follows under #5743.

**Each piece by name, and what the harness does itself (#5750, #5751).** Each piece is a key of
its own beside the counts, `chats[].own_pieces`, recorded as its tool starts it and replaced by
what each `Stop` says is in flight, with `chats[].mid_turn` for a turn under way; `/clear`
clears both. Read from Claude Code 2.1.319's source: `session_crons` holds what `CronCreate`,
`ScheduleWakeup` and `/loop` scheduled, so a wake-up is in it as a one-shot entry. As it resumes
a conversation, Claude Code itself sets again each cron the conversation created that has not
expired and a `ScheduleWakeup` still ahead; of a `ScheduleWakeup` whose time passed, and of
background work its last process left running, it queues a notice of its own, which reaches
the model with the next prompt and starts no turn. So the wake line, which starts the turn,
says the same of what ended and of what was missed ("was due at <time>, missed"), and names a
cron or a wake-up still ahead as the harness's to set again, never the chat's: a chat told to
set it again would set it twice. A chat with nothing else to be told gets no wake line for
those alone, since the harness wakes it for them. A shell's or a monitor's output, a subagent's
transcript and a workflow's output are read at the reopen only where the file's real path is
the one Claude Code keeps for that conversation and that piece, so a chat's own line cannot
have the app read another file into its wake line.
