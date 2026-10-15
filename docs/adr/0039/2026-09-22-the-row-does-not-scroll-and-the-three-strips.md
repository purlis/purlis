# Amendment, 2026-09-22: the row does not scroll, and the three strips behave alike

This record left two of its three open questions to whoever built it. The operator settled both
on 2026-09-22, looking at the running app (`0.1.0-dev.18`):

> "i noticed that tabs now scrollable — instead of automatic expanding in show more button."

That is **the row does not scroll**, and it reverses what this record's author chose while
writing the paragraph above that admits the choice was open. He said it while complaining that
the three strips looked alike — *"tabs should show that PROJECT is holder of workspaces,
workspaces are holder of sessions"* — so it is about all three, which settles the third open
question too: **the three strips behave alike.**

## What this record chose, and why it chose it

Read the two open questions again, because the reasoning is still right and only its conclusion
has moved:

> **Whether the row still scrolls.** The operator said one row with a show-more menu. If the menu
> is what makes the hidden tabs reachable, ADR 0036's scroller has done its job and may go; if
> both stay, there are two overflow mechanisms on one strip.

The implementation kept both, and it kept them for the one thing this record is most insistent
about: **purlis#130 was fifty tabs with no way to reach the last of them**, and the rule that
came out of it is that this strip may not have an unreachable tab. A scroller makes every tab
reachable in the DOM, in the tab order, with its own close button. A menu says there are more. Two
jobs, and `App.css` said so at length.

**The operator is not disputing the constraint. He is disputing that a scrollbar is an acceptable
way to meet it**, and he is right about what he can see: a horizontal scrollbar on a row of tabs
is a control no editor he uses has, it says nothing about how many tabs are past the edge, and
the show-more button beside it made it the second answer to a question one of them was already
answering badly.

## What replaces it, and why the constraint is still met

The collapse is only allowed if it answers #130, so the answer is written here rather than left
to the diff. **Three things, and none of them is a scrollbar:**

1. **Every hidden tab is in that strip's show-more menu, which says how many there are.** That is
   the affordance this record was written to add, doing the job it was added for — only now it is
   load-bearing rather than supplementary.
2. **A menu row brings its tab forward, and the selected tab is always drawn.** So a tab reached
   through the menu arrives *on the strip*, with its own `×`. **This is what keeps the menu from
   becoming the destructive surface this record refuses**: ending a chat is still two presses,
   and there is still no `End chat` under the cursor in a menu that popped up there.
3. **The palette lists every chat with a search and a ranking over it.** Unchanged, and still the
   strongest argument in this record: the palette is the find surface, and the menu is not.

**And the `+` stays outside the collapse**, which is this record's own measured constraint
restated for the new mechanism. It was already outside the scroller; "outside the thing that
hides controls" is the rule, and a collapse is one of those.

## The test that was protecting the old answer, and what it does now

`app/e2e/specs/stress.e2e.ts` closes fifty tabs by pressing the last close button on the strip,
over and over, three rounds of fifty. With the strip scrolling, all fifty buttons are in the DOM
and the loop is trivially correct. With the strip collapsing, the loop still terminates — and
**why** it terminates is the whole argument, so it is now written where the loop is: closing a
drawn tab gives the strip room for a hidden one, so the hidden tabs flow onto the strip as the
drawn ones go.

That is a real dependency and it is worth naming as a hazard: a future change that stopped the
strip re-fitting after a close would leave that loop pressing an empty strip while forty chats
ran. So the spec gained the assertion the collapse makes necessary — **no close buttons AND
nothing behind the show-more button** — and fails there, naming what is left, instead of sixty
seconds later as an unexplained leak.

## How many fit is arithmetic, not a measurement of fifty tabs

The implementation this amendment authorises decides what fits the way **every browser sizes its
own tabs**: each tab takes an equal share of its strip, floored at a minimum and capped. That
makes the question exact — `n` tabs fit in `width` when `width / n >= least` — and the only thing
measured is one number per strip.

This is not a detail, because the obvious implementation is wrong in a way that is hard to see.
The measurement this record's implementation used, an `IntersectionObserver` over every tab
(`app/src/offscreen.ts`), asks which tabs are wholly inside the scroller. That is the right
question about a strip that scrolls and **a loop** on a strip that collapses: hide a tab, its box
is gone, it is not intersecting, and it stays hidden after the room comes back.

It also fixes something this record's implementation had to live with. An intersection is answered
in the engine's rendering step, and **macOS gives a WKWebView no rendering at all while its window
is covered or the display is asleep** (purlis M0.6): `panes.e2e.ts` recorded Linux seeing 3
of 51 tabs and macOS seeing 0 of 49 on the same commit, and the spec had to be written to assert
nothing about which tabs were visible. A `clientWidth` read in a layout effect is a synchronous
layout, and a layout is not a paint — so the scenario spec can now assert what the strip drew.

## What this amendment costs

- **A strip of two tabs draws two wide tabs.** Equal shares of the row is what equal shares means,
  and it is the Zed shape the operator asked for — but it is not the natural-width strip purlis
  drew before, and the difference is most visible with one or two chats open.
- **A name too long for the floor is truncated.** The whole of it is in the tab's tooltip and in
  the palette, which this record already names as the surface for reading rather than aiming.
- **The menu is now the only pointer route to a hidden tab.** It was an affordance beside a
  scroller; it is now a mechanism. A menu that failed to open used to cost an operator an
  affordance and now costs them access — which raises what a defect in it is worth, and the
  measured `pointerdown` defect that this record's implementation already had to work around
  (a WebView click producing no pointer event) is exactly the class of thing that matters more
  now than it did.
- **A keyboard still reaches every tab, and by a different route than it did.** A hidden tab has
  no tab stop on the strip any more. It has a row in the menu, which is a headless primitive's
  roving focus, and it has the palette. That is not worse, but it is not the same, and anyone
  who reasoned about tab order from this record's original text should re-read it here.

## What is left open, still

**Whether a pinned tab is exempt from the collapse.** This record left it open about the
scroller; the implementation's answer — a pin draws a tab first, so it is the last thing to go,
and no exemption of its own — carries over unchanged and is still not what the operator was asked.
