# Amended for #2143, a frame is painted over where it is drawn, 2026-10-18

Spec #2008 rules that what the person typed never reaches the model, in a snapshot or a
screenshot (the operator's ruling of 2026-10-14). The amendments for #2014, item 4, and for #2303
have a screenshot paint over every blanked element and everything that shows a text kept out.
This note says where an element inside a frame is painted; nothing else changes, and the session
protocol does not.

1. **Inside a frame drawn as laid out, the element's own box.** An element inside a frame whose
   document is read in a session of its own is placed by where that frame's element is drawn on
   the page, frame by frame, when each of those frames is drawn upright at the size it is laid out
   at. *D-2143-1.*
2. **Inside any other frame, the whole frame.** When a frame between the page and the element is
   drawn otherwise (turned, scaled or zoomed, by its own element or by anything around it), what
   is painted over for the element is the whole box that frame is drawn in, since everything its
   document draws is drawn within it. *D-2143-2.*
3. **Unsure means withheld.** When the engine does not say where a frame between the page and the
   element is drawn, the screenshot is withheld in words. *D-2143-3.*
