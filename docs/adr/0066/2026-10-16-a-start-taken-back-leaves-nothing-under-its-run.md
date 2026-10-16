# ADR 0066, amended by #2252 (2026-10-16): a start taken back leaves nothing under its run

Decided by the implementer under delegation; the operator can veto.

[The amendment of the same day](2026-10-16-a-start-put-aside-at-once-is-no-run.md) holds a
start's `run.started` until the screen of its project's settings passes. A start put aside
there leaves nothing in the event log under its run id:

- **The sandbox coming back on is held with the run.** A held start whose chat's last run was
  unsandboxed holds its `trust.sandbox.on` with its `run.started`, and the log writes the trust
  event just before the `run.started`, under the same run, when the run is said: once the start
  goes on, or before the first line of the run if one comes first. The run's events after it are
  always after the trust event, as at any start. An opt-out (`trust.sandbox.off`) is never held
  and is still written before its program runs (ADR 0067 §7).
- **The host tells the log a start was taken back.** Its run is then never said, and a later
  line from that program is recorded as a line of a chat the log does not know is: with no
  chat or run, under its number (`chat_number`), and an audit naming the number is of no run. The
  number is not forgotten, so nothing from it begins a run of its own.
- **A run a line of it already said is kept.** The log is append-only: when a line of the
  program reached the log before the start was put aside, its `run.started` is written, and
  what follows of that program stays under that run.

The text above is left as it was written, and this section is the amendment.

## Decisions

- **D-2252-1: the trust event is held in the log with the run.** A
  line of the program can reach the log before the host learns the start goes on; held with the
  run, the trust event is written first whichever comes first.
- **D-2252-2: a start taken back after its run was said keeps that run**, for the reason above.
