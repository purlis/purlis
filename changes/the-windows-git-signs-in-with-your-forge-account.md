### Added

- **The window's git signs in with your forge account.** A save, fetch, push or clone the window
  runs, on a host you signed in to in Settings › You › Accounts, now uses that account through
  purlis's own git credential helper, so it needs no `gh` or `glab`. The helper is named only on
  the command line of the window's git, never in a git config, and no other credential helper,
  your keychain's included, is ever handed the token. A chat's git never gets it (#752).
