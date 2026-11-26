# Amended 2026-11-26: a pressed Accept reaches a running chat's guards

Amends [ADR 0125](../0125-blocks-are-off-by-default-and-opt-in-per-family.md) §3, by the person's
ruling R6 of 2026-11-26 on the prerequisite design
([ADR 0130](../0130-prerequisites-what-a-project-needs-from-the-person-before-its-chats-can-work.md)).

## What changes

§3 lets a start's resolution only tighten what a hook runs: a process that is not the app runs the
stricter of the app's answer and `PURLIS_POLICY` (`enforcement::ENV`), the policy resolved when
the chat started. So a loosening the person pressed reached a running chat only at its next start.

**Once the person presses Accept in the window, a running chat's hook-judged families follow the
accepted policy from its next step.** The hook asks the app, which knows the press, and for a
loosening the person pressed after the chat started, `PURLIS_POLICY` no longer floors the app's
answer. This is the one case where the start's resolution does not floor it.

## What does not change

- **The sandbox level stays as the chat started.** A chat started at the full sandbox keeps it
  until it is restarted; only the families a hook judges follow the press.
- **Only the press lifts the floor.** A loosening the committed file makes with no press still
  waits for it, and nothing a chat puts in its own environment, and no folder it names, loosens
  anything.
- **It fails closed.** Where the app does not answer, the hook runs every family enforced, and
  `PURLIS_POLICY` still floors whatever it runs.
- A tightening still applies at once, with no press.

## Why

The Notice after Accept says "Guards follow it now; a chat started in strict mode keeps the full
sandbox until you restart it" (ADR 0130 §4). Without this amendment its first half was not true for
any chat already running: an Accept made after an app update recovered nothing until every chat
was restarted.

## Consequences

- `enforcement::in_force`'s claim that `PURLIS_POLICY` "can only tighten", the module header's
  rule and `docs/policy.md`'s "the chat's next start runs it that way" change with the code
  (#7724).
- A runner chat follows its starting machine's Accept the same way, once the app answers for it
  (#7722).
