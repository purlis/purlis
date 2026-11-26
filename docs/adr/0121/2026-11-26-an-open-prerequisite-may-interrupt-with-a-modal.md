# Amended 2026-11-26: an open prerequisite may interrupt with a modal

Amends [ADR 0121](../0121-attention-what-waits-on-you-reaches-you-and-nothing-else-interrupts.md)'s
"nothing else interrupts", by the person's decisions recorded in
[ADR 0130](../0130-prerequisites-what-a-project-needs-from-the-person-before-its-chats-can-work.md).

**The one exception.** A **prerequisite** that is open for the project in front opens its modal
without a press: on the project's open, on a policy change mid-session, after an upgrade, and on a
switch to a project whose chip shows its badge. Nothing else opens a dialog, a view or a card on an
arrival; the amendment of 2026-11-12 (Notices opens only on a press) stands.

**Why this one.** Until it is answered, no new chat in the project starts and the running ones work
in strict mode. An entry among the Notices went unread while every refusal named something else
(#7714).

**What it is not.** A prerequisite is not a **Blocking ask**: it is never in Asks and adds nothing
to the ✋ count or the Dock badge. It has its own signals: the modal, the project chip's badge, its
Notice, and the CLI's and `purlis doctor`'s words.

**What keeps it from being noise.** One modal at a time, never over another dialog; it opens again
only when the person asks for it (the Notice's main action) or the prerequisite changes. Work in
strict mode holds until the app restarts or the policy changes.
