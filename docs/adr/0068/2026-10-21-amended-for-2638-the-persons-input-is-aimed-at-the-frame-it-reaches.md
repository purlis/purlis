# Amended for #2638, the person's input is aimed at the frame it reaches, 2026-10-21

1. **Input is checked where it lands.** The person's input names what a frame showed: the page,
   and its document number. Where the page still shows that number, the input goes on. Where a
   frame of it loaded another document since, the input goes on only if it lands on a document
   that has not: a key in the document that has the focus (the page's own where nothing does),
   a press, a move or the wheel in the document under its point, found by the engine's own hit
   test (`DOM.getNodeForLocation`, and `DOM.describeNode` where the point is on a frame's
   element). That frame, every frame around it, and for a point on a frame's element every frame
   inside it, must not have loaded another document since. Where a frame of the page went since,
   no point is known to be on what the person saw, so a pointer event is dropped; a key is
   checked by the focus as before. Anything the engine cannot say drops the input, and the bar
   says so as before (#2304). *D-2638-1, decided in implementation, to be confirmed.*
