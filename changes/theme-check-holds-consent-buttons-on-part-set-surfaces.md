### Fixed

- **A panel, popover or dialog a theme fills holds the buttons drawn on it.** When a theme's
  parts give one of these surfaces a fill of its own, the buttons on it, and a menu's rows in a
  popover, are measured on that fill in every workspace tint: their words, Allow's accent edge
  and the danger answers. A surface they would not read on draws its own fill, and
  `purlis theme check` names it (#3099).
