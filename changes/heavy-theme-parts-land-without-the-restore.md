### Fixed

- **A heavy theme's parts land when they are measured, not when the restore ends.** The first frame
  says its parts are coming and the window asks for them as it starts, so a slow project restore no
  longer leaves a theme drawn in its family defaults, and an ask that fails is made again (#4410).
