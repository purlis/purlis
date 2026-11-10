### Fixed

- **↑ and ↓ at the edge of a wrapped line stay in the box** (#6045). In a reply box in a list's
  row, ↓ with the caret at the end of a line its words wrap from (as End puts it, in a word too
  long for one line) left the row for the one below. It now moves the caret on to the next line,
  and ↑ and ↓ leave the row only from the box's first or last line.
