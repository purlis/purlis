### Fixed

- **A title-bar control whose colours fail in two states at once falls back only there**
  (#4049, #4064). A workspace chip whose theme colours can't be read while it is both current
  and under the pointer now draws its default only in that case: under the pointer alone, or
  current alone, it keeps the theme's colours. A switcher row's see-through fill is checked on
  the popover colour the theme sets, not on the default popover.
