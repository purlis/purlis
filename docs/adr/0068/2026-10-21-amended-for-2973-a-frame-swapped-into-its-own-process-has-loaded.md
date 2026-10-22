# Amended for #2973, a frame swapped into a process of its own has loaded, 2026-10-21

1. **A swap is a load.** When the engine attaches a session of its own for a frame purlis knew
   in another session (the frame moved to a process of its own, as a frame navigated to another
   site does), the frame counts as having loaded another document from then: the page's document
   number moves, and so does the frame's (`loaded`), whatever else the engine says of the swap.
   What was dealt or blanked in the frame's old document is forgotten, as after a navigation.
   Input aimed at what was shown before is judged against the frame as it is now (#2638).
   *D-2973-1, decided in implementation, to be confirmed.*
