### Fixed

- **The theme check reads a pane's warning buttons on the warning wash.** A pane's warning and a
  consent's Allow are drawn on `wash.warning` over the terminal's fill, but the check never read
  a control there, so a theme with a heavy warning wash could hide a warning's ways out and still
  pass. Their looks are now held on that wash as they are on the danger wash. No built-in theme
  changes (#4795).
