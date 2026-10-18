### Fixed

- **A click or drag inside a frame the page turns, scales or zooms lands where the element is
  drawn.** The agent's click, hover or drag aimed at an element inside a frame from another site
  lands on the element where it shows on screen, however the page transforms the frame, and only
  inside the frame that shows the element. When purlis cannot tell where the frame is drawn, or
  the element lies outside what its frame shows, the act is answered "not shown" (#2557).
