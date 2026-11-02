### Fixed

- **The theme check reads more of what stands on a surface in the tints.** A see-through button
  look on an opaque popover, panel or dialog fill is now read in each workspace tint, and so is a
  look only a set of states draws, such as hovered and pressed at once. A custom theme that fell
  short there now draws the surface's family default, and the check says why (#4607).
