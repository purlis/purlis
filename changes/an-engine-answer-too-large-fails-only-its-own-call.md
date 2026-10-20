### Fixed

- **A page too large to read fails only the call that read it.** An answer from the browser
  engine past the most one call may take now fails that call with a sentence, and the browser
  keeps answering; before, it ended the browser. The read that marks what a box doesn't show is
  held to a smaller bound, so a page heavy in inline script or attribute text has nothing marked
  by it (#2694).
