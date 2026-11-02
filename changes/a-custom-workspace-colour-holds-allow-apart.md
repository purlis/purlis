### Fixed

- **A workspace's own colour measures Allow's edge as it is drawn.** Under a theme that tints Keep
  blocked's edge, the accent could give way to the waiting amber and land a shade too near that
  edge, at a hue only a custom `#rrggbb` reaches. Every rule is now measured at the chroma each
  side is drawn at (#3388).
- **Yellow and green workspaces keep the accent apart from the waiting amber.** On Purlis light
  (hues 90 to 117) and Purlis's high-contrast dark (hues 92.75 to 148), the window tinted the
  accent and the edges drawn in it, which put them too near the amber. It now draws them as
  written, as the theme engine always measured them (#3388).
