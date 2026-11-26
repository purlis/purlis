# Amended 2026-11-26: a pressed Accept reaches a running chat's guards

Amends [ADR 0125](../0125-blocks-are-off-by-default-and-opt-in-per-family.md) §3, by the person's
ruling R6 of 2026-11-26 on the prerequisite design
([ADR 0130](../0130-prerequisites-what-a-project-needs-from-the-person-before-its-chats-can-work.md)).

## What changes

§3 lets a start's resolution only tighten what a hook runs: a process that is not the app runs the
stricter of the app's answer and `PURLIS_POLICY` (`enforcement::ENV`), the policy resolved when
the chat started. So a loosening the person pressed reached a running chat only at its next start.
Two more floors are drawn at the same start, and held a running chat the same way:

- **the chain's**: the strictest policy along the chat's dispatch chain, which the app recorded
  when the chat was dispatched (`reopen::HandedFrom::policy`) and applies to its answer
  (`enforcement::Resolved::for_chat`, in the app's `Ask::Policy`);
- **the forge route's**: what the chat started under, which the route takes when it is made
  (`forge::mergestance::PolicyMergeStance`, `merge_mode`'s `started`).

**Once the person presses Accept in the window, a running chat follows the accepted policy from
its next step, in all three places.** Its hook-judged families, the policy a task it dispatches
starts with, and its forge route's `guards.merge` all run the accepted policy, so the three never
disagree. The hook asks the app, which knows the press; for a loosening the person pressed after
the chat started, `PURLIS_POLICY` no longer floors the app's answer, the chain's recorded policy
no longer floors it where every chat above it is in this project by the app's record, and the
route's `started` no longer floors its `merge_mode`. These are the only cases where the start's
resolution does not floor it.

**Only the press lifts the floor, and it fails closed.** The app's Policy answer says whether the
person pressed Accept for this project since the asking chat started, by the app's own record of
both; only then does the hook, the chain and the route run the answer without the start's floor.
An answer without it (an older app) keeps the floor. A chain the app cannot place wholly in this
project keeps its floor.

**The press is kept in the app's memory only.** After the app restarts, chats still running in
the session host get their start's floor back until they restart. This fails closed, and the
window's Notice for such a chat says so.

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
- Nothing new is written to disk: the press is a fact of the app's run (ADR 0069 needs no tier).

## Why

The Notice after Accept says "Guards follow it now; a chat started in strict mode keeps the full
sandbox until you restart it" (ADR 0130 §4). Without this amendment its first half was not true for
any chat already running: an Accept made after an app update recovered nothing until every chat
was restarted. Lifting the hook's floor alone would have left a dispatched task and the forge route
strict while the hook ran the accepted policy, so the chat would be told two things at once.

Keeping the press on disk would let a file loosen a running chat; keeping it in memory means a
restart can only put the stricter floor back.

## Consequences

- `enforcement::in_force`'s claim that `PURLIS_POLICY` "can only tighten", the module header's
  rule, `PolicyMergeStance::of`'s "a loosening press reaches it with the chat's next start", and
  `docs/policy.md`'s "the chat's next start runs it that way" change with the code (#7729).
- `hookwire`'s Policy answer gains the fact of the press; an app that does not send it is read as
  no press.
- A runner chat follows its starting machine's Accept the same way, once the app answers for it
  (#7722).
