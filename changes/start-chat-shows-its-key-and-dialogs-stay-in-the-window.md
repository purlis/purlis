### Added

- **A theme can style the key a button shows.** Start chat now shows the key that presses it
  (⌘↩ on a Mac, Ctrl+Enter elsewhere) after its words. A theme sets that key with the button
  family's new `shortcut` part, `"button.shortcut.text"` and `"button.shortcut.font-size"`, which
  `purlis theme parts` lists and the published schema completes. The theme specimen draws it, so
  `theme_preview` shows it. The key is hidden while the button cannot take it, during an approval,
  a start without the sandbox, and while the chat starts (#3377, #3510).
- **Start a chat's Name field names the next chat's number**, `steward 10`, rather than "steward
  and its number". The number is a guess, and the chat still takes the number it is dealt (#3376).

### Changed

- **Every dialog sits at a fixed top, 6% down the window and never closer than 24px, and keeps
  24px clear of the bottom.** In a short window the dialog grows shorter, so its answers stay in
  the window, Start chat's included (#3389, #3510).
