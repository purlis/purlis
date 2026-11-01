### Fixed

- **A theme with a tint recipe and see-through colours no longer holds up the window for its whole
  measure at launch.** The first frame waits about 50 ms for the theme's parts, where it used to wait
  hundreds of milliseconds per appearance, and draws those parts in their family defaults until the
  measure lands (#4337).
