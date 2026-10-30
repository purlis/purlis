### Fixed

- **A control whose theme colours fall back keeps its hover, pressed, selected, refused and
  unavailable looks** (#4040). When a theme's colours for a control at rest can't be read and
  are drawn back as the default, the control's other looks (under the pointer, pressed or
  chosen, a refused field's danger edge, an unavailable control's faint words) are no longer
  covered by that default. Each one draws as its own default, and anything the theme set for
  that look itself is kept. This was already the case for the title bar; it now holds for every
  kind of control.
