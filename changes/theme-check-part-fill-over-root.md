### Fixed

- **The theme check measures a see-through part fill where it is drawn.** A part's fill with
  alpha, such as `rail.badge.fill`, is now laid over its root's own fill and then the grounds
  under that, so words that can't be read there draw the defaults back, and words that can
  are kept (#3303).
