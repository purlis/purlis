### Added

- **A theme says how a workspace's colour tints it** (#2329, #2393). `theme.json`'s `tint` can
  turn the tint off, name which tokens a workspace's colour turns, and give any of the eight
  workspace colours a hue of its own. A theme that extends another keeps whatever it does not
  say. The window draws the recipe the theme engine hands it, and the theme check measures
  every tint the theme draws. Purlis Classic's recipe is today's.

### Changed

- **A workspace's colour keeps the accent apart from the danger colour.** Where a tinted accent
  would sit too near danger, the tint keeps the workspace's hue and gives it less colour. In
  Purlis Classic's light appearance this softens the accent of red, orange and pink workspaces.
