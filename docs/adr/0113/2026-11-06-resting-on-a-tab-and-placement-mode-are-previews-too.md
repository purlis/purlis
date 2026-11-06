# Amendment, 2026-11-06: resting on a tab and placement mode are previews too

Two of this record's exceptions close (#5294, #5293). The rest of it stands.

- **Resting on a tab is a preview** (#5294). A drag that rests on another tab for about 650 ms,
  a starting value from the prototype, draws that tab in front and tells the core nothing of it:
  not the layout, not the view tabs, and no strip remembers it as its front tab. What the window
  draws is told once the drag is over, so a drop into that tab is one change, and a cancel, which
  draws the tab that was in front again, tells nothing. The tab wears a mark while it is about to
  come forward, the pane preview family's `opening` part. The tab already in front takes no rest.
- **Why the window's tabs still move, and not a second copy drawn beside them.** The window draws
  its tab in front from a dozen readings of its tabs (crumbs, bands, asks, the strip), so a front
  drawn from anything else would have to be passed to each of them, and one missed would draw the
  wrong pane's marks. Holding back what is *told* keeps one source for what is drawn.
- **Placement mode draws the same preview as a drag** (#5293), from the same store and layer: the
  target the keyboard is on is the place the pointer would be over, at once, with the same words,
  sizes and marks. Its crosses sit on the zones the drag measured, so they stay put as the panes
  make room. The box over a pane and the dashed held pane are gone. A press on a tab of the strip
  while something is held brings it forward the same way, and Esc puts back the panes, the tab in
  front, its focused pane and the keyboard. Placement mode shows no grabbing or no-drop cursor.
