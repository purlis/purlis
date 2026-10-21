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
2. **Of several elements the tree marks focused, the document's own is the one.** A real engine
   may mark more than one element of a page focused in its accessibility tree (a field's inner
   editor, or the document, with the field). Then each of their documents is asked, in
   purlis's world, which element has the focus; the one element both name is blanked, and a page
   where no one element is named still holds the key back (`FocusUnclear`). *D-2295-3, decided
   in implementation, to be confirmed.*
3. **What is measured, and what is held to the bound.** The time from a key to the page is said
   on CI for the first key into the field and for each key after it. The median of the keys
   after the first is held under the spec's 50 ms; the first key, which reads the whole tree,
   and the worst key, which moves with a shared runner's neighbours, are said and not held.
   *D-2295-2, decided in implementation, to be confirmed.*
