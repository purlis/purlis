### Fixed

- **Tab in a list's row works one way in both directions** (#6071). In the Inbox and every list
  whose rows hold controls, one Tab from a row leaves the list, past the row's buttons and boxes,
  as one Shift+Tab does. From a button in a row, Tab now moves on to the row's next control and
  Shift+Tab back to the one before, as they already did from a box; past the last or the first,
  both leave the list.
