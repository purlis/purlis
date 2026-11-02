### Changed

- **A frame is judged by the origin of its document.** While a browser's reach is judged, what
  the agent types, presses or drops into a frame is judged by the origin of the frame's
  document, as the browser gives it: a `blob:` frame by the origin that made it, an
  `about:blank` or `about:srcdoc` frame by the origin it takes from the page around it. A frame
  with no web origin of its own, such as a `data:` frame, is judged as opaque, and nothing is
  typed into it.
