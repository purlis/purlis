### Added

- **A theme is a folder, and may have several appearances** (#2322). A theme's `theme.json`
  names a published schema, so an editor completes every token name; it may `extend` another
  theme and hold only what differs, name its colours once in a `palette`, and carry one or more
  appearances, each dark or light and optionally high contrast. A project's theme set to follow
  the system now follows it with your own theme, when that theme has a dark and a light
  appearance. A theme that breaks the format is refused whole, and purlis draws its own theme
  and says what is wrong. Today's single-file themes keep loading as they are, and today's look
  ships as **Purlis Classic**, unchanged.
