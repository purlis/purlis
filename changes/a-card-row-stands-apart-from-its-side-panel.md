### Fixed

- **A side view's card rows read as cards in Purlis Classic.** In Classic dark, the raised ground
  was almost the side panel's own, so the cards read as plain rows. A card row now stands on a
  control's ground, and on a control's hover ground under the pointer. Every theme draws that
  ground apart from the window and from a panel, because a button stands on both. Purlis keeps
  its cards on the window's ground, set on its panels. The theme check reports a theme whose card
  rows step less than 1.05:1 from the side panel they stand on and draw no edge a hairline from
  it. It changes none of the theme's colours for that (#3774).
