### Added

- **A chat can put a vault secret into a field of its browser, with your yes.**
  `browser_fill_secret` names a field from the page, a vault and a key; you see the key, its
  vault, the exact site, the page and the field's label on one notice, and choose Allow once or
  Keep blocked. purlis fills it only into a one-line field whose own frame is on that site at the
  moment of filling, and the value never reaches the chat (#5115, spec #5100).
