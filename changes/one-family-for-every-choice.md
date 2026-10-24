### Changed

- **Every checkbox, radio, switch, segmented control and slider is drawn one way.** A ticked box,
  a picked radio and a switch that is on take the accent's fill with a mark in the window's colour
  on it, and nothing beside a choice moves when it changes. A switch is a 34 × 20 pill whose thumb
  slides across in 150 ms in Purlis, and never under reduced motion. Settings' You / Project / Workspace
  switch and the Chats and Inbox scopes are one segmented control: the chosen segment stands on the
  window's ground with a thin edge under it, and its words keep their weight. Start a chat's harnesses are cards
  of one height with no radio circle, the chosen card told by its wash, its accent edge and its
  check; a persona's icon and colour are picked from tiles. Every group of options is one Tab stop
  with the arrow keys moving the pick, and the Chats list's state chips stay one Tab stop too. A
  theme can set each choice through its parts (`switch.track.selected.fill`,
  `option-card.root.card.selected.border-colour`, `segment.root.selected.fill`), and
  `purlis theme parts` lists them (#2907).
