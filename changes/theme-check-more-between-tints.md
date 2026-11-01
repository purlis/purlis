### Fixed

- **`purlis theme check` reads the selected row, a card row and a notice's way between the named
  tints** (#4406). Each is now read across every hue a workspace's own colour can be, and said once
  where it falls only between the named tints. New findings: `selected-between-tints`,
  `card-between-tints` and `notice-way-between-tints`.
- **A see-through count on a tab is measured over its counts button** (#4406), so its words are no
  longer judged on the tab's fill, a colour they are never drawn on.
