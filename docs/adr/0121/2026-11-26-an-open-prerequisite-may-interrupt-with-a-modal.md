# Amended 2026-11-26: an open prerequisite may interrupt with a modal

Amends [ADR 0121](../0121-attention-what-waits-on-you-reaches-you-and-nothing-else-interrupts.md)'s
"nothing else interrupts", its decision 2 and its decision 16, by the person's decisions recorded
in [ADR 0130](../0130-prerequisites-what-a-project-needs-from-the-person-before-its-chats-can-work.md).

**"Nothing else interrupts", with one exception.** A **prerequisite** that is open for the project
in front opens its modal without a press: on the project's open, on a policy change mid-session,
after an app update, and on a switch to a project whose tab in the title bar shows its dot. Nothing
else opens a dialog, a view or a card on an arrival; the amendment of 2026-11-12 (Notices opens only
on a press) stands.

**Why this one.** Until it is answered, no chat the person starts in the project starts, and the
running ones keep the policy in force (strict mode under Broken). An entry among the Notices went
unread while every refusal named something else (#7714).

**Decision 2: a prerequisite is not a Blocking ask**, though it holds a chat's start and a `purlis`
command until the person answers. It is never in Asks and adds nothing to the ✋ count or the Dock
badge, and the notifier sends no system notification for it. Its signals are its own: the modal, the
dot on the project's tab in the title bar, its Notice, and the CLI's and `purlis doctor`'s words.

**Decision 16: a command waiting on a prerequisite names the window, not Asks**: "waiting for the
person: <project>'s policy needs your answer in the purlis window on this machine, for up to 90
seconds".

**What keeps it from being noise.** One modal at a time, never over another dialog, and one per
window. It opens again only when the person asks for it (the Notice's main action) or the
prerequisite changes; a change back to a digest already answered this run does not open it. The
answer that grants nothing holds until the app restarts or the digest or reason changes.
