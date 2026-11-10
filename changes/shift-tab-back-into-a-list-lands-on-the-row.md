### Fixed

- **Shift+Tab back into a list lands on the row you left, not on a button or box in it** (#6033).
  In the Inbox and every list whose rows hold controls, a row whose controls came last in the
  list used to give Shift+Tab to its last control, so a screen reader announced that control
  with no row around it. Shift+Tab now lands on the row itself, and → reaches its controls
  again.
