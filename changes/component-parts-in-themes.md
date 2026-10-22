### Added

- **A theme can set one part of a control.** A theme's `theme.json` takes a `parts` object of
  keys `<family>.<part>[.<variant>][.<state>].<property>`: `"button.root.corner-radius": "full"`
  draws every button as a pill, and `"button.root.primary.hover.fill": "accent.base"` changes only
  the primary under the pointer. `purlis theme parts` lists every family, part, state and
  property with its bounds, and the published schema completes each key. A state never changes a
  part's size, a control stays at least 24px tall with its words fitting, the floors hold for
  each part, and Allow keeps its edge and never takes the primary look. A theme that names no
  parts draws exactly as before (#2903).
