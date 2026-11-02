### Changed

- **A page's own document is judged by its origin.** While a browser's reach is judged, what
  the agent types, presses or drops into a page is judged by the origin of the page's own
  document, as frames already are: a blank page another page wrote by the writer's origin, a
  `blob:` page by the origin that made it. A page with no web origin of its own, such as a
  `data:` page, is judged as opaque, and nothing is typed into it; a blank page whose origin
  is not known yet is not typed into until it is. The browser's own error page sends nowhere.
- **A sandboxed frame is judged as opaque by its sandbox.** An `about:blank` or `about:srcdoc`
  frame whose own document has not said its origin is judged by its frame element's `sandbox`:
  sandboxed without `allow-same-origin`, it is opaque and nothing is typed into it.
