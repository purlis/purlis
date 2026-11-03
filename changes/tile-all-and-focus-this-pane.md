### Added

- **Tile all evens a tab's panes into a grid.** The tab's right-click menu and the palette offer
  it, and a split tab shows it at the right end of the tab strip. It arranges only the panes
  already in the tab, in reading order, and "Undo the last layout change" in the palette puts
  them back. In a tab with one pane the menu and the palette show it greyed (#4765).

### Changed

- **⌘⇧Enter is now Focus this pane.** In a split it zooms the focused pane to fill the tab, and
  the tab shows a zoom mark until the same key brings the other panes back. In a tab with one
  pane it is Focus mode, as before (#4765).
