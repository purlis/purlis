### Changed

- **Every button in the window is one button family** (#2904). Buttons, icon buttons and
  buttons that read as links are drawn and behave the same everywhere: one look per variant
  (primary, secondary, ghost, danger, soft accent, soft warning, link, and the grant's look an
  Allow keeps), three sizes, and every button in a row the same size. Every Allow and Accept
  leads with its open lock and its accent edge, and is never drawn as the suggested answer. The
  window's buttons sit on React Aria, so they press the same way from a pointer, a key or a
  screen reader, and show their focus ring only from the keyboard.
