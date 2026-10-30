# The budget leaves Use skill…'s row, and the chip warns of every chat need

**Amends [ADR 0102](../0102-a-skill-router-decides-what-a-chat-sees-and-nothing-it-hides-is-out-of-reach.md), 2026-10-30**, tickets #3953 and #3954 (spec #3679 S8). It replaces what [The skill budget and Use skill…, as built](2026-10-21-the-skill-budget-and-use-skill-as-built.md) said of where the budget is shown and warned of.

## Where the budget is said

S8 makes the sidebar Skills view the one answer to what a chat gets. The budget is said there: in the line under whose set it shows, and as a Needs you card when the set is near or past it.

- **The pane's Skills chip** still shows how many skills the set holds, and opens the Skills view on the chat.
- **The Use skill… row** (the palette's, for the chat in front, and each chat tab's menu row) says only "Use skill…". It no longer says the budget, and it does not warn.
- **Use skill…** is a quick picker and says neither.
- Where the pane leaves the chip out (beside a task's breadcrumb, or in a pane 40rem wide or less), the Skills view (`⌘⇧X`) says the budget. The tab's row no longer does.

## What the chip warns of

The chip turns warning-coloured whenever the Skills view's Needs you holds a card of the chat's own:

- its set near or past the budget (the 80% warning stays);
- its pins read by none;
- its router fell back;
- skills it started without;
- its skills changed since it started.

The chip and the view reckon these with one function, so they cannot disagree. Its tooltip names each need. Knowing them adds no read to a chat's open:

- `chat_skills`, which the chip's budget always came from, also says what the chat started without. The core holds that from the chat's start.
- What changed comes from the `chats_plane_updated` answer the window already reads for its tabs' marks.

## Use skill…'s key

Use skill… gains a key, `⌘⇧S` (`Ctrl+Shift+S` off a Mac). It opens Use skill… on the chat in front: in a split tab, on the pane that has the keyboard.

**Built by #3953 and #3954.**
