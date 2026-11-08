### Changed

- **The title bar's crumb and a switcher row take a control's corner.** They draw
  `radius.control`, as a workspace chip does, instead of a step above `radius.md`. Purlis and
  Purlis Classic look the same as before. A theme that extends neither draws them at its
  control's corner, or sets `titlebar.crumb.corner-radius` and `titlebar.row.corner-radius`
  (#3196).
