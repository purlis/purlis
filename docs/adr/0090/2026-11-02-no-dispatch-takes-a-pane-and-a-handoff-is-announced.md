# Amended (2026-11-02): no dispatch takes a pane, and a handoff is announced (#4752)

Records rulings the person made on 2026-11-02 on the map
[Chats, tabs and panes: a chat isn't a tab](https://github.com/purlis/purlis/issues/4575), gathered
in spec [#4750](https://github.com/purlis/purlis/issues/4750): the
[charting decisions](https://github.com/purlis/purlis/issues/4576) D1, D2 and D4,
[A window with no tab per chat](https://github.com/purlis/purlis/issues/4588) rules 4 to 6,
[Storing a tab's layout](https://github.com/purlis/purlis/issues/4587) rule 4,
[Several windows](https://github.com/purlis/purlis/issues/4701)' `looked_at`, and the level-1
dispatch the person ruled on [Rewriting the records](https://github.com/purlis/purlis/issues/4704).
This records the decision ahead of the behaviour, which ships with that milestone's build.

## Every dispatch starts in the sidebar

**Dispatched tasks and handoffs start in the sidebar only, and never take a pane.** A chat the
person starts takes the focused pane; a chat another chat starts never rearranges their screen.
Every chat is listed in the sidebar, so "listed" no longer needs a tab: §1's "a tab on the target
workspace's strip" for a handoff, and "headless" for a task, both become **in the sidebar, in no
pane**. "Headless" is retired as a word for a chat: every chat is either shown in a pane or not.

## A handoff is announced in three places

Q11 kept "visibility is the control" as listed, observable and stoppable. A handoff's tab was also
its announcement, the safety property `crates/purlis-core/docs/handoff.md` gave it: a chat started
by another chat always lands where the person can see it. That property is now carried by three
things, for every handoff, including one into the workspace in view:

- **A card on the target workspace's title-bar chip**: "<chat> arrived from <asking chat>", with
  **Go to it**. It never takes focus, and pressing elsewhere leaves a dot on the chip. With no chip,
  it is listed in the Notices centre with a way to open that workspace.
- **An Update in the Inbox.** It is not counted on the ✋, because a handoff is not an ask.
- **A "new" mark on its sidebar row** until the person has looked at the chat.

The 2026-10-07 amendment's "a handoff opens as a tab, as before" and its "until AC-12, a
handed-off chat still opens on a strip" are replaced by this.

## §7's level-1 guard retires

§7 opened a level-1 dispatch as a tab, because a level-1 chat cannot say that it is waiting and a
headless one could wait for ever where nobody looks. **The person ruled that a level-1 dispatch
also starts in no pane**, and is announced like a handoff: the chip card, the Inbox Update and the
row's "new" mark. Its row keeps the faint hand that means it can't report asks, and the dispatch's
reply tells the asking chat that the person must look at it, so the asking chat can say so. "Only
at level 2 or 3" no longer holds.

## Pressing, holding and closing a task

- **A pressed task lands like any chat** (the 2026-10-07 amendment's "shows it inside the tab of
  the session that asked for it"): it replaces what the focused pane shows, or is jumped to if it
  is already on screen. The breadcrumb path in its pane walks up the chain in that same pane.
  **Reopen** of a finished task is a press too: it takes the focused pane, where the 2026-10-08
  amendment gave it "an ordinary chat with a tab".
- **The end is held while the task is looked at** (the 2026-10-08 amendment's "while the task is
  the chat in front"): while it is in a pane of the front tab of any of the project's windows on
  screen. A press sends its new layout in the same call, so a task the person presses onto is held
  from that moment.
- **A close ends no task, and needs no sending back.** Close pane clears the pane and the task
  keeps working in the sidebar; "the cross and the close shortcut on a task's own tab send the
  task back to the Chats list" is what any close does now.
- **Ending an asking chat** asks once what to do with the chats it started, where it said
  "closing". A needs-you mark rolls up to the chat that asked and to the workspace's chip, where it
  said "the workspace's tab".

## Asks from a chat in no pane

A blocking ask from a chat in no pane never takes a pane or focus. It shows on the chat's row, the
rows above it, the title bar's ✋ and the workspace chip's count, and the person answers it in the
Inbox. If the chat is on screen, its pane band shows the ask too, and it stays open there even with
notices minimised. A task's questions still go to its asking chat first (#4551).

Where this record's amendments say a mark, an ask or a Notice is shown "on the chat's tab" or "the
task's tab", read the chat's pane band when it is on screen, and its row, the ✋ and the Inbox when
it is not. The wording sweep at the milestone's end rewrites them
([#4770](https://github.com/purlis/purlis/issues/4770)).
