# Amended for #2295, typing while driving stays quick, 2026-10-21

1. **A field already blanked is not read for again while it keeps the focus.** The first key
   that may change a field reads the page's tree and blanks the element that has the focus, as
   before (#2014). Until the focus moves, a later key asks only the field's own document, in
   purlis's world, which element has the focus where that document has it
   (`document.hasFocus()`, `document.activeElement`, `DOM.describeNode`): the same element goes
   on as blanked, and anything else, nothing focused, an answer that does not come, a press of
   the mouse, a change of document, a take-over or a hand-back, reads the tree again. What the
   field holds is still read from the field after every key and kept out. The time from a key
   to the page on a large page is measured on CI. *D-2295-1, decided in implementation, to be
   confirmed.*
