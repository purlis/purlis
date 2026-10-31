### Fixed

- **When one look of a control can't be read, only that look falls back** (#4174). If a theme's
  colour for a control under the pointer, pressed, chosen or focused can't be read, purlis now
  draws the default only where that look holds. Before, it reached further and could take away
  colours the theme set for the control's other looks, for example a button's focus edge, or the
  fill of a row under the pointer that is also focused. This was already the case for the title
  bar; it now holds for every kind of control.
