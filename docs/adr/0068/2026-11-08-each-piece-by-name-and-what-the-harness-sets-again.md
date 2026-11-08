# Amended (2026-11-08): each piece of own work by name, and what the harness sets again itself

This follows the amendment of the same day, ["A reopened chat is told what the restart ended"](2026-11-08-a-reopened-chat-is-told-what-the-restart-ended.md), as #5750 and #5751 built it.

Each piece is a key of
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
