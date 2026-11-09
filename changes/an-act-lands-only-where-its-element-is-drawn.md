### Changed

- **A chat's browser act lands only where its element is drawn.** A click, hover or drag on an
  element that another element or frame is drawn over is refused as not shown, and nothing is
  pressed; a click on an element its own label or a box around it is drawn over still lands. A
  click reads again what is drawn at its point once the mouse is there, and an act on a page's
  own document waits for its scroll to be drawn before it is aimed.
