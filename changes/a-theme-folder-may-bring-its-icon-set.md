### Added

- **A theme folder may bring its own icons for the rails** (#2337). `theme.json`'s `icons`
  names a file in the folder that draws each view's tab as path data, stroked the way the
  rails draw their icons today. A theme that extends another keeps every icon it does not
  redraw. Purlis Classic keeps today's icons. The Purlis theme's set is Lucide's icons, chosen
  to match the new look's.
