### Changed

- **The waiting colour is never taken for the accent.** A theme's accent, and the edges drawn
  in it (the focus ring, a selected row's edge, the primary's edge), stay visibly apart from the
  waiting amber, as written and in every workspace colour. In a yellow or an orange workspace the
  accent gives way: it is drawn with less colour, or as the theme writes it where no amount
  holds, while the accent's washes keep the workspace's colour. A theme that sets the two too
  close is drawn with purlis's own waiting colour, and `purlis theme check` says so (#3130).
