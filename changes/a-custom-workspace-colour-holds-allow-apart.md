### Fixed

- **A workspace's own colour measures Allow's edge as it is drawn.** Under a theme that tints Keep
  blocked's edge, the accent could give way to the waiting amber and land a shade too near that
  edge, at a hue only a custom `#rrggbb` reaches. Every rule is now measured at the chroma each
  side is drawn at. No built-in theme's colours change (#3388).
