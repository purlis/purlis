### Fixed

- **A theme reaches a switcher row's count, and the title bar's controls are held where they
  are drawn** (#3947, #3949). A switcher row's waiting count is now `titlebar.row-count`, apart
  from a chip's `titlebar.count`, so a theme colours it. The crumb, the ⌄, and a switcher's rows
  and pins are checked against their contrast floors on the ground each is drawn on, under the
  pointer, open or pinned; one whose colours a theme set too faint draws its own default back,
  and no longer takes the workspace chips' colours with it.
