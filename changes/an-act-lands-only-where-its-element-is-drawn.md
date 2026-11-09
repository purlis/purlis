### Changed

- **A chat's browser act lands only where its element is drawn.** A click, hover or drag on an
  element that another element or frame is drawn over is refused as not shown, and nothing is
  pressed. A click still lands through a box the pointer passes through, and on an element whose
  label, label's `::before`, or a box around it (a web component's around its slot included) is
  drawn over it. A click reads again what is drawn at its point once the mouse is there, and an
  act on a page's own document waits for its scroll to be drawn before it is aimed.
