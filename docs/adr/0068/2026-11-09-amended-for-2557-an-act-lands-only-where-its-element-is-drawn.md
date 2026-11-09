# Amended for #2557, an act lands only where its element is drawn, 2026-11-09

The amendment for #2557 says where an act aimed at an element lands. This note says what must be
drawn at that point before anything is pressed; nothing else changes, and the session protocol
does not.

1. **What is drawn at the point stands for the element.** Before a click, hover or drag sends
   anything, the engine is asked what is drawn at the point, followed frame by frame into the
   element's own document. It must be the element, something inside it (its shadow roots and what
   it draws itself included), an element around it, or a label whose control it is: a press on
   any of those reaches the element. Anything else, including another document's frame drawn over
   it, and anything the engine cannot answer, is answered "not shown" and nothing is pressed.
   *D-2557-6, decided in implementation, to be confirmed.*
2. **Read again at the press.** A click reads what is drawn at its point again once the mouse is
   there, just before the press. Between that read and the press is one engine round trip.
   *D-2557-7, decided in implementation, to be confirmed.*
3. **Aimed once the scroll is drawn.** An act on an element of the page's own document, once it
   is scrolled into view, waits for the document to draw two frames before it is aimed, as an act
   inside frames already does. *D-2557-8, decided in implementation, to be confirmed.*
