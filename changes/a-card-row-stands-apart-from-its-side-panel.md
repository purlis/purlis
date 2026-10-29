### Fixed

- **A side view's card rows read as cards in Purlis Classic.** In Classic dark, the raised ground
  was almost the side panel's own, so the cards read as plain rows. Classic now draws a card row
  on its sunken ground, a step from the panel in dark and in light. The theme check reports a
  theme whose parts put a card row less than 1.05:1 from the side panel it stands on, with no
  edge a hairline from it. The check only reports it and redraws nothing (#3774).
