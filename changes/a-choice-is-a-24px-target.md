### Fixed

- **A checkbox, chip, radio, switch, slider or small segmented control is never under the 24px
  target size in the compact density.** Each is at least 24px tall at any text size, as a small
  button and a small field are: a slider takes a press anywhere in that height, and a segment
  anywhere in its control's (WCAG 2.5.8, #4173). In the compact density, a small list's row with
  a checkbox in it (a todo in the Todos panel) is 24px tall now, where it was 18.
