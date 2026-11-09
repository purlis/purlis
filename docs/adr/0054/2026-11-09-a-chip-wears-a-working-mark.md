# Amendment, 2026-11-09: a workspace's chip wears a working mark (#3127)

[The title bar's amendment](2026-10-24-the-workspaces-move-to-the-title-bar.md) let the strip's
dot go with the strip, so nothing in the title bar said which workspace had a chat at work. The
steward ruled on [#3127](https://github.com/purlis/purlis/issues/3127), under the operator's
standing order of 2026-11-08, that **a chip keeps a mark while a chat in its workspace is
working**. The prototype is kept on `prototype/3127-3128-titlebar-chips` (its variant A).

**What it is.** While any chat the plane files in a workspace is in its turn (`working`, or with
its own work in flight: `chatState.inItsTurn`), that workspace's chip wears **the chat mark's
working ring** (`ui/chat-mark`, the glyph the chat wears on its tab and its row) on its lower
trailing corner, opposite the notice dot, ringed in the bar's own ground. The plane root's house
and the dashed chip wear it too; a dormant chip never does, since its workspace is gone.

- **A shape, never a colour alone** (WCAG 1.4.1), held at 3:1 on the bar's ground as a mark.
- **Said**: the chip's description adds "1 chat working" or "N chats working" after what waits
  and its notices. Its name stays its words (WCAG 2.5.3). "Working" is the run state's word; the
  window never says "running chat" (CONTEXT.md).
- **Still, and moves nothing.** It never spins and never plays an arrival; it is laid over the
  chip's slot, so it coming and going moves no chip and no neighbour (R153). A press goes through
  it to the chip, and it is no stop of the strip's arrows.
- **Moved by its cause only** (ADR 0076): a chat's hooks say where its turn is; nothing reads
  what a chat prints. It counts every chat the plane lists, tab or none (#4758).

**For the record.** The dot the strip drew before #2916 was a LIVE workspace's mark
(charter-app#301, published with the plane), not a chat at work. That dot stays gone from the
title bar; the explorer still marks a LIVE workspace.
