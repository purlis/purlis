# Amended for #2840, out-of-view marks follow every box around a frame, 2026-10-20

This takes the place of D-2689-1 and D-2689-2 (the boxes of the frame element's document only,
and an origin inset by the border alike on each side).

1. **An element's part is carried out to the page.** An element's part is cut to each box it is
   in, then to what its frame shows (the frame element's content box: its client box less its
   padding), then carried into the frame element's document and cut again, outward through
   every frame to the page. A frame from another site, read in a session of its own, is carried
   the same way through the layout of each session around it, nearest first, each cut to what
   its own layout viewport shows. An element of which nothing is left is marked out of view.
   *D-2840-1, decided in implementation, to be confirmed.*
2. **A frame is placed by its content box at the scale it is drawn.** The layout read asks for
   each element's border and padding widths, side by side, and its transform. A frame's
   document starts inside its element's own border and padding, and is drawn at the element's
   drawn size over its laid-out size (border, client box, border). *D-2840-2, decided in
   implementation, to be confirmed.*
3. **A turned frame is not carried through.** Where the frame element, or a box around it in its
   document, is turned or skewed (a transform other than a move or a positive scale; any 3D
   matrix), the layout's bounds no longer say where the frame's document is drawn, so nothing
   in it is said to be cut off by what is around it. *D-2840-3, decided in implementation, to
   be confirmed.*
