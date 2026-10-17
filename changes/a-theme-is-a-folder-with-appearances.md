### Added

- **A theme is a folder, and may have several appearances** (#2322). A theme's `theme.json`
  names a published schema, so an editor completes every token name; it may `extend` another
  theme and hold only what differs, name its colours once in a `palette`, and carry one or more
  appearances, each dark or light and optionally high contrast. A project's theme set to follow
  the system now follows it with your own theme, when that theme has a dark and a light
  appearance. Today's single-file themes keep loading, and today's look ships as
  **Purlis Classic**, unchanged.

### Changed

- **A theme with a value purlis cannot read is refused whole** (#2322, ADR 0097 §2), whichever
  format it is in and wherever it comes from: purlis draws its own theme and the Notice names
  each value, instead of drawing the theme with that value replaced. A token a theme does not
  name is still purlis's, silently.
