### Added

- **`purlis theme list` and `purlis theme check`.** `list` shows every theme available on this
  machine with where it comes from: built in, your own `theme.json` (`mine`), or an approved
  extension. `check <theme>` reports a theme's errors, its contrast warnings (each pair with its
  ratio, in every workspace tint) and its hard floors, as text or as `--json` for a chat, and
  exits 1 on an error. `purlis docs show themes` has the details (#2321).
- **An extension's theme says what is wrong with it.** What purlis had to put right in an
  approved extension's theme, or that it is not JSON at all, is shown on its row in Extensions
  instead of being dropped (#2321).
