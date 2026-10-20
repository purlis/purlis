# Amended for #2689, a box around a frame from another site cuts it off, 2026-10-20

A frame from another site runs in a session of its own, so the engine's layout of the page around
it (`DOMSnapshot.captureSnapshot`) does not hold its document, and the layout of its own session
does not hold the boxes its element is in.

1. **What a frame from another site shows is cut to the boxes around its element.** Its
   elements are compared with what its own layout viewport shows (its scroll and size), cut to
   the part of its element that the boxes on the page around it let show. That part is read from
   the layout of the session that holds the element (`DOM.getFrameOwner`), at the element's
   client size inset by its border, and is held to the same rules as a box in one document: an
   element taken out of the flow, and the page's root and body, are not compared. A box hiding the
   whole frame marks all of its elements. Only the document the element is in is read, not a
   frame further out. Each session's layout is read at most once per look. *D-2689-1, decided in
   implementation, to be confirmed.*
2. **A frame's document may end in another session than it started in.** The document of a
   frame from another site is asked for in the session of the page around it, and the engine
   answers it in the frame's own session, under the same id. A request for a document is ended
   by its end in any session of the page, since a document's id is its load's own token; any
   other request is ended only in its own session, since those ids are numbered per process.
   Before this, a page holding such a frame was never settled. *D-2689-4, decided in
   implementation, to be confirmed.*
