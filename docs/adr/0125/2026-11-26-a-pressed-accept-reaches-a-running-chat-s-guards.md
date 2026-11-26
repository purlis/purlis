# Amended 2026-11-26: a pressed Accept reaches a running chat's guards

Amends [ADR 0125](../0125-blocks-are-off-by-default-and-opt-in-per-family.md) §3, by the person's
rulings R6 and R10 of 2026-11-26 on the prerequisite design
([ADR 0130](../0130-prerequisites-what-a-project-needs-from-the-person-before-its-chats-can-work.md)).

## What changes

§3 lets a start's resolution only tighten what a hook runs: a process that is not the app runs the
stricter of the app's answer and `PURLIS_POLICY` (`enforcement::ENV`), the policy resolved when
the chat started. So a loosening the person pressed reached a running chat only at its next start.
Two more floors are drawn at the same start, and held a running chat the same way:

- **the chain's**: the strictest policy along the chat's dispatch chain, which the app recorded
  when the chat was dispatched (`reopen::HandedFrom::policy`) and applies wherever it judges for
  that chat (`enforcement::Resolved::for_chat`): its hook's answer, what a task it dispatches
  inherits, its brokered commits, its vault checks and its masking;
- **the forge route's**: what the chat started under, which the route takes when it is made
  (`forge::mergestance::PolicyMergeStance`, `merge_mode`'s `started`).

**Once the person presses Accept in the window, a running chat follows the accepted policy from
its next step, in all three places.** Its hook-judged families, the policy a task it dispatches
starts with, and its forge route's `guards.merge` all run the accepted policy, so within one run
of the app the three never disagree. The hook asks the app, which knows the press; for a loosening
the person pressed after the chat started, `PURLIS_POLICY` no longer floors the app's answer, the
chain's recorded policy no longer floors it where each chat above it is found, by id, among this
project's open chats, and the route's `started` no longer floors its `merge_mode`. These are the
only cases where the start's resolution does not floor it.

**Only the press lifts the floor, and it fails closed.** The app's Policy answer says whether the
person pressed Accept for this project since the asking chat started, by the app's own record of
both; only then does the hook, the chain and the route run the answer without the start's floor.
An answer without it (an older app) keeps the floor. Each chat above it is found, by id, among
this project's open chats; one that has closed or is not found keeps the floor (fail closed), and
so does a chain through another project.

**The press updates the chain's record (R10).** When the person presses Accept, the app clears the
strict inherited chain policy (`HandedFrom::policy`, `app/reopen.json`'s `chats[].from.policy`) of
each running chat in this project that the press lifts. It is the only thing the press writes,
and only the person's press causes it. The record is the app's, and no chat can write it. So a
dispatched chat follows the accepted policy now, and at its restart, before or after an app
restart (its start resolves with no chain to floor it). A chain whose chat above has closed or is
not found, or that runs through another project, is not lifted, and its record is untouched.

**The start is recorded (R11).** Each running chat's start policy, as `PURLIS_POLICY` spells it
(`enforcement::Resolved::spelled`), is kept in the app's session record: a `policy` on the host
record's `Started` (`crates/host/src/hostrecord.rs`), written when the chat starts, never by the
press and never from anything a chat sent. It has its own row and tier in `docs/plane-format.md`.

**The fact of the press is kept in the app's memory only.** After the app restarts, every chat
still running gets its start's floor (`PURLIS_POLICY`, and the route's `started`) back until it
restarts. The re-attached forge route takes its `started` from the recorded start, and the
window's Notice for such a chat reads it and says so. A dispatched chat the press lifted gets its
start's floor back as any chat does, and that floor holds the chain's policy it started with
(`PURLIS_POLICY` folds it in). Its record stays cleared, so its restart starts under the accepted
policy.

## What does not change

- **The sandbox level stays as the chat started.** A chat started at the full sandbox keeps it
  until it is restarted; only the families a hook judges, its chain and its route follow the
  press.
- **Only the press lifts the floor.** A loosening the committed file makes with no press still
  waits for it, and nothing a chat puts in its own environment, and no folder it names, loosens
  anything.
- **It fails closed.** Where the app does not answer, the hook runs every family enforced, and
  `PURLIS_POLICY` still floors whatever it runs.
- A tightening still applies at once, with no press.
- The format of `app/reopen.json` does not change: the press empties a field it already has
  (Clone state, ADR 0069; its row in `docs/plane-format.md` says so). The host record gains one
  optional field, `started.policy` (R11), absent from a line an older host wrote, which reads as
  no recorded start: the floor is then the strictest, the full sandbox with every family enforced.

## Why

The Notice after Accept says "Guards follow it now; a chat started in strict mode keeps the full
sandbox until you restart it" (ADR 0130 §4). Without this amendment its first half was not true for
any chat already running: an Accept made after an app update recovered nothing until every chat
was restarted. Lifting the hook's floor alone would have left a dispatched task and the forge route
strict while the hook ran the accepted policy, so the chat would be told two things at once.

Keeping the fact of the press on disk would let a file loosen a running chat's start floor; keeping
it in memory means an app restart can only put that floor back. The chain's record is different: a
dispatched chat restarts from it, so leaving it strict would put a chain the person lifted back on
every restart of the task. It is the app's own record, written only on the press, so no file a
chat can write loosens anything.

## Consequences

- `enforcement::in_force`'s claim that `PURLIS_POLICY` "can only tighten", the module header's
  rule, `PolicyMergeStance::of`'s "a loosening press reaches it with the chat's next start", and
  `docs/policy.md`'s "the chat's next start runs it that way" change with the code (#7729).
- `hookwire`'s Policy answer gains the fact of the press; an app that does not send it is read as
  no press.
- `docs/plane-format.md`'s `chats[].from.policy` row names the press as the only thing that
  empties it while the chat keeps its `from`, and a new row records the host record's
  `started.policy` (R11) with its tier.
- A runner chat follows its starting machine's Accept the same way, once the app answers for it
  (#7722).
