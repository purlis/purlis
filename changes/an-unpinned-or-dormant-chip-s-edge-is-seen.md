### Fixed

- **An unpinned or dormant workspace chip's edge is easy to see** (#3999). The title bar draws the
  current workspace that is not pinned, and a pin whose workspace is gone, with no fill of its own:
  their dashed and dotted line is what shows them. That line was the window's faint hairline, which
  in Purlis dark reached only 1.38:1 against the bar. It is now drawn in the chip's own muted words,
  at least 3:1 in every built-in theme and in every state (WCAG 1.4.11).
