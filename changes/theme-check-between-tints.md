### Fixed

- **`purlis theme check` reads a pair between the named tints** (#4308). A workspace's own colour
  can be any hue, so a pair a tint can move is now read at every whole degree. A new finding,
  `contrast-between-tints`, names a pair that falls only between the named tints, at the degree it
  is lowest.
