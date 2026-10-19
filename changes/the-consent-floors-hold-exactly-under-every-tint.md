### Changed

- **The consent floors hold exactly under every workspace tint** (#2526). A tinted shade keeps
  the contrast it had untinted at every hue, custom colours included: each channel is rounded on
  the side of the shade's own luminance. The focus ring, the selected row's edge, the accent's
  surface and the active tab stay apart from the danger colour as the accent does, and a grey
  accent keeps as much of its tint as holds it apart. In Purlis Classic most tinted colours move
  by one step of a channel or two, too little to see.
