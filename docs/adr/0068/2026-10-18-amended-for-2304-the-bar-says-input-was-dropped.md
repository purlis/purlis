# Amended for #2304, the bar says when the person's input was dropped, 2026-10-18

The amendment for #2014, item 5, has the engine drop an event of the person's whose page has moved
on in any of its frames, so input aimed at one document never lands on another's. This note says
how the person learns of it; the engine's rule, the session protocol and the host do not change.

1. **Said in the bar, once, until it is done again.** When a key the person pressed, or a mouse
   button they pressed or let go, is dropped because the page moved on, the bar purlis draws above
   the page says so and says to type or click again. However many are dropped in a row, it is one
   line, and it goes once a key or click of theirs reaches the page, the browser is handed back,
   or the view is watched again. *D-2304-1, decided in implementation, to be confirmed.*
2. **A move or a wheel is not said.** A mouse move or a wheel turn that is dropped is followed by
   the next one, so the bar says nothing of it. *D-2304-2, decided in implementation, to be
   confirmed.*
3. **Aimed by the whole page, still.** Input stays aimed at the page and all its documents, not at
   the one frame under the point or the focus: a change in another frame still drops it, and the
   bar says so. *D-2304-3, decided in implementation, to be confirmed.*
