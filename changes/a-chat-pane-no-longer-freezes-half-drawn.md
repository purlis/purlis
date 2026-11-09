### Fixed

- **A chat's pane no longer freezes half-drawn after the waiting strip comes and goes.** A
  program on the alternate screen, in a pane made shorter and taller again, could make the
  terminal throw while drawing, after which it drew nothing more and took no keys until the chat
  fell out of the last three shown. The terminal now keeps the alternate screen to its rows, and
  a pane whose terminal stops drawing what it is sent starts its terminal again on its own
  (#5478).
