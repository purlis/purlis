### Added

- **A theme sets its geometry and fonts** (#2326). A theme folder may give spacing, type sizes,
  weights, line heights, corners, control and strip heights, the focus ring and the two lifts a
  number inside the range purlis documents for each, and bring its window, code and terminal
  fonts as woff2 files in its own `fonts/` folder, which purlis reads and hands the window
  itself. A value outside its range refuses the theme, and the Notice names the range.
- **Density and fonts are yours, in every theme** (#2326). Settings › You › Density and fonts
  draws the window comfortable or compact, and takes a family of your own for the window, for
  code and for the terminal, which beats the theme's. A theme may tune what compact means for
  it; Purlis Classic's compact is tighter spacing and shorter controls and strips.

### Changed

- **The terminal is drawn in the theme's terminal font**, which for Purlis Classic is the
  family it always had.
