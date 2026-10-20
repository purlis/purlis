### Fixed

- **Out-of-view marks follow every box around a frame.** A snapshot or a find marks a frame's
  element `(out of view)` when any box around the frame hides it: a box on the page, or one in
  a frame further out. The frame is placed inside its own border and padding, at the size the
  page draws it, for frames from the page's own site and from others alike (#2840).
