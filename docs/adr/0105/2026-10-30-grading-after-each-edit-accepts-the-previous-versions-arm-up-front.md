# Amended 2026-10-30: grading after each edit accepts the previous version's arm up front

Ticket #4035. It adds to the 2026-10-30 amendment that a grade after an edit compares with the
version before each edit.

**The plan that turns grading after each edit on shows and keeps the chats with the previous
version's arm counted** (`after_edit::chats_to_accept`), even where the skill has no previous
version yet. Each saved edit is committed, so a skill with one version gains a previous version
at its first edit, and every grade after an edit runs that arm from then on. Before this, the
plan showed two arms and kept their chats, so the first edit of every new skill paused the
opt-in and asked the person to turn it on again.

A copy only on this machine (`skills.local/`, or a workspace that is not live) is never committed,
so it never gains a previous version, and its plan counts no third arm.

The plan says why its number is larger than a grade's now ("Your first saved edit makes a
version of … to compare with, so each grade after an edit also runs that version"), and the
turn-on button's description names the larger number. The cap and the dollar limit are
unchanged: they hold each grade whole, however many chats it runs.

## Consent stays strict

- A grade after an edit still never runs more chats, or may spend more, than the plan the person
  pressed showed. The larger number is shown before they press, and is what they accept.
- An opt-in kept before this, on two arms, still waits at the first edit. Its reason now says
  that the edit made a version to compare with, so the grade also runs that version.

## Rejected

- **Dropping the ask without showing the third arm.** The arm is more chats. Spend is held by the
  same cap and limit, but chats are part of what the plan shows and the person accepts.
- **Accepting the plan's two arms and letting the third run unasked.** The same as above, once
  removed.
