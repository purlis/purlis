### Added

- **A chat can fill a 2FA code from a seed in your vault.** `browser_fill_secret` with `code`
  names the key that holds the seed (an `otpauth://totp/` address or the base32 secret); you
  allow it for one site, once or for the chat, on the same notice as any secret, and purlis works
  the current code out as it fills. Only the code reaches the page; the seed never leaves the vault's side.
