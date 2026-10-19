### Fixed

- **A click, hover or drag inside frames nested below another frame's fold reaches its
  element.** Where the element is not shown on the first read, purlis scrolls each frame that
  holds it into view, outermost first, and reads again before it answers "not shown". An act
  lands only at a point the page's own window shows (#2690).
