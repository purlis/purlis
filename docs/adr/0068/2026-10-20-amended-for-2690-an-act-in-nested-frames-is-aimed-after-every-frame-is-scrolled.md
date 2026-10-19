# Amended for #2690, an act in nested frames is aimed after every frame is scrolled, 2026-10-20

The amendment for #2557 says where an act aimed at an element inside a frame lands. This note
adds two rules; nothing else changes, and the session protocol does not.

1. **Every frame is scrolled before "not shown".** When the element's centre is not inside what
   each frame between the page and the element shows, on the read taken after the element is
   scrolled into view in its own document, each frame element is scrolled into view, outermost
   first, and where the frames are drawn is read again. Only a second read that still does not
   show the element is answered "not shown". The scrolls around the element's point and the
   reads until the frames are at rest (#2557, items 4 and 5) then follow as before.
   *D-2690-1, decided in implementation, to be confirmed.*
2. **Only where the page's window shows.** An act aimed at an element (a click, a double
   click, a hover, either end of a drag onto an element, the start of a drag by an offset) lands
   only at a point at least half a pixel inside the page's own layout viewport, the area input
   events are dispatched in. A point outside it, or a viewport the engine does not report, is
   answered "not shown" and nothing is sent to the page. *D-2690-2, decided in implementation,
   to be confirmed.*
3. **A drag by an offset ends where the agent says.** The end of a drag by an offset is the
   agent's own point on the page, as before, and is not held to the window.
   *D-2690-3, decided in implementation, to be confirmed.*
