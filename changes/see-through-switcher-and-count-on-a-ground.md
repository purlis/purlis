### Fixed

- **A custom theme's switcher rows and plain count are read as they are drawn.** A popover filled
  opaque is read in the workspace tints when a switcher row on it draws a see-through fill, so its
  words can no longer fall in a tint unseen; and a see-through rule on `activity.count`, or a
  see-through `needs-you.count`, is held apart from the hot count as the two come out on the bar,
  not as they are written (#4607).
