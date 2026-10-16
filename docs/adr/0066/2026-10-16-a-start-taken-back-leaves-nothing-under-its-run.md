# ADR 0066, amended by #2252 (2026-10-16): a start taken back leaves nothing under its run

Decided by the implementer under delegation; the operator can veto.

[The amendment of the same day](2026-10-16-a-start-put-aside-at-once-is-no-run.md) holds a
start's `run.started` until the screen of its project's settings passes. A start put aside
there leaves nothing in the event log under its run id:

- **The sandbox coming back on is written for the start that goes on.** A held start whose
  chat's last run was unsandboxed writes its `trust.sandbox.on` once its start goes on, just
  before its `run.started`, under the same run. The chat is sandboxed from its first byte, so
  nothing runs unrecorded meanwhile; an opt-out (`trust.sandbox.off`) is never held and is still
  written before its program runs (ADR 0067 §7).
- **The host tells the log a start was taken back.** Its run is then never said, and a later
  line from that program is recorded as a line of a chat the log does not know is: with no
  chat or run, under its number (`chat_number`). The number is not forgotten, so such a line
  never begins a run of its own.
- **A run a line of it already said is kept.** The log is append-only: when a line of the
  program reached the log before the start was put aside, its `run.started` is written, and
  what follows of that program stays under that run.

The text above is left as it was written, and this section is the amendment.

## Decisions

- **D-2252-1: written for the start that goes on, not held in the log.** The trust event is the
  host's to write through the same callback as any start's; holding it in the recorder too
  would give the log two held kinds to keep in step.
- **D-2252-2: a start taken back after its run was said keeps that run**, for the reason above.
