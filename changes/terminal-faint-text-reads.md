### Fixed

- **Faint text in the terminal is readable in every appearance.** Words a program draws faint
  in the terminal's own colour now take the theme's faint colour, which holds 4.5:1 against
  the terminal's background, instead of being drawn at half opacity. Themes can set it with
  the new `terminal.faint` token, and an imported theme gets one made from its own colours.
