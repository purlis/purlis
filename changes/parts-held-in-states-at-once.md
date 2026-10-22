### Changed

- **A theme's button parts are held in states that hold at once.** A button hovered and pressed,
  or focused and selected, is measured as the stylesheet draws it, against the same floors as one
  state; a part that breaks one there draws its default, and the theme check names the states
  (#2958).
