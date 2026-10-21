# Amended for #2294, taking over is tested end to end on a real engine, 2026-10-20

The person taking a browser over (#2014) is driven on a real `purlis serve` and the pinned
Chrome for Testing, through the chat's tool server and the window's view, by the test #2243 built
(its amendment of this date). This note records one rule the real engine showed.

1. **The element with the focus is the innermost one that says so.** A real engine says that a
   page's document has the focus while a field in it has it too. Before each key that edits, the
   focused element is blanked, and two focused elements read as a page that cannot tell which
   field has the focus, so the person's first key after the agent clicked a field was held. A
   document the engine says has the focus is dropped only where a focused element is in its own
   frame or a frame nested in it. A document focused beside an element in another frame stays,
   and the two still read as unclear, so the key is held. An element the engine names twice
   counts once. *D-2294-4, decided in implementation, to be confirmed.*
