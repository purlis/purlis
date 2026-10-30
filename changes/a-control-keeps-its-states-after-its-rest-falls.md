### Fixed

- **A control whose theme colours fall back keeps its hover, pressed, selected, refused and
  unavailable looks** (#4040). When a theme's colours for a control at rest can't be read and
  are drawn back as the default, the control's other looks are no longer covered by that
  default: the control, and the words of its label, icon and other parts, draw as the default
  of each look, including a look that is selected and unavailable at once. Colours the theme
  set for a look are kept wherever that look holds. This was already the case for the title bar;
  it now holds for every kind of control.
