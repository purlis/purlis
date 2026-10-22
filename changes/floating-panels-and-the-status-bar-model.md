### Added

- **A theme sets the window's gutter and its panels' corners.** Three geometry tokens join each
  appearance: `window.gutter` (the gap between the regions, 0 to 16 px), `panel.corner-radius`
  (a region panel's corners, 0 to 24 px) and `panel.border-width` (the hairline between two
  flush regions, 0 to 2 px). The Purlis theme floats its panels 8 apart with 12px corners and no
  hairline; Purlis Classic is 0, 0 and 1, and draws flush as before (#2913).
- **You can hide any status bar item.** Right-click the status bar, or press the menu key or
  Shift F10 on one of its items, and uncheck an item to hide it; check it to show it again. Your
  choice is kept on this machine. purlis's own items and an extension's are on the bar by the
  same rules: left or right, highest priority first (#2913).
- **A theme colour for warnings that ask for care**: `wash.warning`, the amber wash the
  waiting pill stands on, in every built-in appearance (#2913).
- **Themes reach panels, sections, empty states and status items.** The parts manifest is at
  version 4, with the `panel`, `panel-section`, `empty-state` and `status-item` families
  (#2913).

### Changed

- **The title bar's right end is status pills**: the needs-you hand, the save, About, the kill
  switch and the update offer are each a 24px pill with full corners and 12px words (#2913).
