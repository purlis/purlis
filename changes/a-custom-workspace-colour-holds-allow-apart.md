### Fixed

- **A workspace's own colour measures Allow's edge as it is drawn.** Under a theme that tints Keep
  blocked's edge, the accent could give way to the waiting amber and land a shade too near that
  edge, at a hue only a custom `#rrggbb` reaches. The same search also took colour from the accent
  it did not need to. Every rule is now measured at the chroma each side is drawn at. In Purlis's
  light appearance, a custom workspace colour of hue 98 to 117, between yellow and green, now draws
  the accent near a grey rather than in Purlis's violet; the eight named colours are unchanged
  (#3388).
