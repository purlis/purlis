# Amendment, 2026-10-22 (#2913): a box is no target, and its corners are still clamped

The window's own families join the manifest at version 4: the region panel, a panel's section,
an empty state and a status item. Three of them are boxes as tall as what they hold, not
controls. §3 is read for them as follows (the dispatcher's decision on #2913):

- **A box is no target.** A panel, a section and an empty state are held to their bounds alone:
  the 24px least target and the words fitting in a height are a control's, and they grow to hold
  their words. A status item is a control, 24px tall in either density, and keeps both.
- **Every part that draws a box keeps the clamp at half its height.** A control's corner is
  clamped as the engine writes it, to half the height it is drawn at. A box has no height the
  engine knows, so each of its corners is written as `min(<r>, 50%)`, which the browser resolves
  on the box's own size per axis: one value or four, no corner passes half the box. `full` on a
  box is `50%`.
- **The window's geometry is the theme's tokens**: `window.gutter`, `panel.corner-radius` and
  `panel.border-width` (#2805), which a panel's `corner-radius` part refines for one variant.
