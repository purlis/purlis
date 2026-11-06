# Panes make room while a drag holds a preview, not a change

**Accepted 2026-11-05**, from the person's acceptance of the prototype on
`prototype/moving-panes` (spec #5288, milestone M71 · Moving panes). It amends three rulings, named
below.

## Where purlis was

While a pane was dragged, the person saw a coloured box over a pane (#4583's verdict for #4763) and
had to imagine the layout that would result. A chat's row had its own "Place…" button (#5067). A new
chat replaced what the focused pane showed, even in a tab the person had split (#4750).

## The decision

- **The panes move to where a drop would leave them, and what is moved shows in its place.** The
  drop model stays the one source of what a drop does: the preview is `dropOn`'s result for the
  place under the pointer, laid out as the window lays out a tab. A place `judgeDrop` refuses has no
  result, so a preview can't promise what a drop won't do. It adds no rule.
- **The window holds a preview, not a change.** The real tabs don't change during a drag. Nothing
  reaches the core, is saved or goes to a terminal until the drop. Esc, a release on no place, the
  pointer being taken away and the window losing focus let go of what the drag wrote, and put back
  the tab in front, the focused pane and the keyboard as they were before the press.
- **A real pane moves, by a transform and a size written on its frame.** No tree is drawn again, so
  no terminal is taken out of its pane and what prints goes on printing. What has no frame on
  screen, a chat from the list or a side that comes back unfolded, is a stand-in pane; a chat that
  takes a pane's place covers it, and the old chat keeps running behind.
- **A terminal's grid isn't resized until the drop, and is resized once then.** The element a
  terminal's observer watches is pinned to its pixels for the drag, so however its frame moves the
  observer hears nothing. Its text keeps its width, and its frame clips it or spaces it. Letting go
  of the pin is the one change it hears. A label reckons the size each will take from the would-be
  box and the terminal's own cell, as its fit reckons it.
- **Drop zones stay on the layout from the drag's start.** They are measured once, and retaken only
  when another tab comes to the front, a pane comes or goes under the drag, or the room changes, so
  panes making room can't move a zone under the pointer.
- **Stickiness, a wait and a duration.** A place holds about 3% past its line, a target changes
  after about 70 ms, and the panes take `duration.enter`, none under reduced motion. Each is a
  starting value: a measured one replaces it without a new decision.
- **Over its own place, a drop is its own kind of verdict:** neither allowed nor refused. It draws
  no danger and no mark, and the words say letting go leaves it where it is.
- **A notice that arrives during a drag waits.** A pane's top keeps its height until the drop, so the
  notice waits on the bell and in the Inbox rather than moving the layout under the pointer.
- **A press is only heard by the project in front.** A project kept behind has panes of its own
  numbers, and no drag of a project in front is its to act on.

## What this amends

- **#4583's verdict for #4763, a box over the pane,** becomes panes making room. The box stays in
  placement mode until #5293 draws this preview there too, and a line still says where a tab would
  go on the strip.
- **#5067's "Place…" on a chat's row** goes (#5290), and the row's menu keeps a Move row, so no route
  is lost.
- **#4750's opening rules** change where they say a new chat replaces a pane (#5289, `opensAt` in the
  tabs model):
  - **Story 1** (a press shows the chat in the focused pane) now holds only in a one-pane tab. In a
    split, a press off screen gets a new tab, and an empty pane fills first.
  - **Story 5** (with a view focused, a press goes to the most recently used pane showing a chat or
    nothing) now holds only with a view alone in the front tab, and only to an empty pane or the chat
    of a one-pane tab. A view in a split gets a new tab, like any split.
  - **Story 12** (New chat takes the focused pane) is reversed: a new chat fills an empty pane of the
    front tab, or gets a new tab, and replaces nothing.
  - **Story 51** (a press still replaces the focused pane inside a pinned tab) holds for a one-pane
    tab. A pinned split gets a new tab.
  - **Story 63** (a shell takes the focused pane) still follows a chat's rules, so it opens as a new
    chat does.
  - **The summary's "A new tab appears only when the person asks for one"** no longer holds: a chat
    the person opens can get one.
  - **"A press's target" in the layout model**, "otherwise the focused pane, unless it holds a view",
    gives way to `opensAt`.
  - **ADR 0038 and ADR 0061** say the same in their amendments of 2026-11-02, and each has an
    amendment of 2026-11-05 for it; so does ADR 0043's "a press on a chat" line.
  - **Kept:** story 4 (a press never replaces a view), a reopen of a finished task landing by the
    press rule, and story 87 (⌘⇧J's pick shows in this pane), a breadcrumb, ⌘⇧H and a drop, which
    name their own pane.

## What it costs

- **The motion's in `docs/design-system.md`'s table,** and "the terminal is left still" still holds
  for what it shows: its frame moves, and its grid and its text don't.

- **A pane's frame leaves the boxes of its layout during a drag,** so those boxes clip nothing while
  it lasts. That is one rule in `paneFrame.css`, and the library's own inline `overflow` is why it
  says `!important`.
- **A split takes the tree's share when a drop changes it under a live group.** The library reads a
  side's size once, as it mounts, and a drop can leave other panes on the same two sides, so the
  window would otherwise draw the old sizes where the preview promised the tree's.
- **Resting on a tab still brings it forward by a real change** until #5294, which is told to the
  core like any change of tab. A cancel puts it back.
- **The feel of it is the person's to judge on hardware:** a terminal's WebGL surface moved and
  clipped, WebKit's cursors and animation, and a screen reader on the words. Each is filed
  `needs-hardware` in the milestone.
