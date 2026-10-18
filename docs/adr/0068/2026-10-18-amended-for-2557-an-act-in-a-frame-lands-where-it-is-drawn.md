# Amended for #2557, an act inside a frame lands where the element is drawn, 2026-10-18

The amendment for #2013 gives the agent clicks, hovers and drags aimed by reference, and the one
for #2143 says where an element inside a frame is painted in a screenshot. This note says where an
act aimed at an element inside a frame lands; nothing else changes, and the session protocol does
not.

1. **Where the element is drawn.** An act aimed at an element inside a frame whose document is
   read in a session of its own lands at the point where the element's centre is drawn on the
   page. Frame by frame, from the element's document outwards, the point is carried through how
   that frame's element is drawn: moved, for a frame drawn upright at the size it is laid out at;
   otherwise through the map that takes the frame's laid-out box to the four corners the engine
   says it is drawn at, which holds for every turn, scale, skew, zoom and perspective. *D-2557-1,
   decided in implementation, to be confirmed.*
2. **A drag by an offset is an offset on the page as shown.** The offset of a drag is in CSS
   pixels of the page, the way the screenshot the agent reads shows it, wherever the drag starts.
   *D-2557-2, decided in implementation, to be confirmed.*
3. **Unsure means not shown.** When the engine does not say where a frame between the page and
   the element is drawn, or draws it with no area, with its corners crossed, or with a corner
   behind the eye of a perspective, the act is answered "not shown" and nothing is sent to the
   page. *D-2557-3, decided in implementation, to be confirmed.*
4. **Only inside the frame that shows the element.** An act lands only inside the frame that
   shows its element: when the element's centre is not at least half a pixel inside what each
   frame between the page and the element shows of its document, the act is answered "not shown"
   and nothing is sent to the page. Before an act the element is scrolled into view in its own
   document, then each frame between, outermost first, is scrolled to show the point where the
   element is in it, and where each frame is drawn is read again after those scrolls.
   *D-2557-4, decided in implementation, to be confirmed.*
