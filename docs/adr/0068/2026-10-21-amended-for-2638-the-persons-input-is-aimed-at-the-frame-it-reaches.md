# Amended for #2638, the person's input is aimed at the frame it reaches, 2026-10-21

1. **Input is checked where it lands.** The person's input names what a frame showed: the page,
   and its document number. Where the page still shows that number, the input goes on. Where a
   frame of it loaded another document since, a key goes on only if the document that has the
   focus has not: the one the tree marks focused (of several, the one its own document names,
   #2295), or the page's own where the tree marks none and its document has nothing focused
   either. A move of the pointer goes on only if the document under its point has not, found by
   the engine's own hit test as the engine routes the event (`DOM.getNodeForLocation`, a box the
   pointer passes through not hit; `DOM.describeNode` where the point is on a frame's element,
   and an element that holds a document but names none drops the move). That frame, every frame
   around it, and for a point on a frame's element every frame inside it, must not have loaded
   another document since. Anything the engine cannot say drops the input, and the bar says so as
   before (#2304). *D-2638-1, decided in implementation, to be confirmed.*
2. **Only a move goes on while the page has moved on.** A press, a release or the wheel is
   dropped whenever any frame of the page loaded another document, or went, since the person
   saw it: the hit test says only what is under the point now, and what a frame that moved on
   draws may be moved under the point between the check and the event. A move is checked as
   above, and once the window shows the page as it is, every event goes on again. *D-2638-2,
   decided in implementation, to be confirmed.*
3. **A frame first seen counts from then.** A frame attached since the person saw the page counts
   as loaded at the page's document number when purlis first saw it; a page whose number has not
   moved takes input exactly as before. *D-2638-3, decided in implementation, to be confirmed.*
4. **A known limit.** A page's own key handler may move the focus into another frame while a key
   is on its way; that key's character lands where the handler put it. Any check made before a
   key is sent has this limit, and the page that moved the focus has the key already.
