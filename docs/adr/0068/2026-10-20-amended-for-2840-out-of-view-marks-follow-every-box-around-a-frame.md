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
   drawn size over its laid-out size (border, client box, border). The engine's bounds and
   client boxes are fractional, and its border and padding widths are whole device pixels where
   it snaps them, so a frame is placed to within about a pixel; only what lies wholly outside is
   marked, so that error moves a mark only for an element within a pixel of a box's edge.
   *D-2840-2, decided in implementation, to be confirmed.*
3. **A turned frame is not carried through.** Where the frame element, or a box around it in its
   document, is turned or skewed (a `transform` other than a move or a positive scale, any 3D
   matrix, a `rotate` of any angle but nothing, a `scale` with a factor of nothing or less, or
   any `offset-path`), the layout's bounds no longer say where the frame's document is drawn, so
   nothing in it is said to be cut off by what is around it. An element its own document's boxes
   or its frame already hide is still cut off: nothing left is nothing left, whatever is around.
   *D-2840-3, decided in implementation, to be confirmed.*
4. **An element of no width or no height is given a thousandth of a pixel.** Its part is then
   cut as a line's would be: one lying on a box's edge, or past it, has nothing left and is cut
   off, and one inside the box is not. *D-2840-4, decided in implementation, to be confirmed.*
