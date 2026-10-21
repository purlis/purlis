### Changed

- **A theme with component parts is measured once.** Its stylesheet, the theme check and the
  gallery's cards share one pass over the workspace tints, so applying a parts theme and drawing
  a gallery of them take about half the time (#2954).
