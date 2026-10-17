### Added

- **`purlis theme list` and `purlis theme check`.** `list` shows every theme available on this
  machine, with its appearances and where it comes from: built in, your own `theme.json`
  (`mine`), or an approved extension. `check <theme>` (an id, or the path of a theme file or
  folder) says why a theme is refused, what was left out of it, its contrast warnings in each
  appearance (each pair with its ratio, in every workspace tint) and its hard floors, as text or
  as `--json` for a chat, and exits 1 when the theme is refused. `purlis docs show themes` has
  the details (#2321).
- **An extension's theme says what is wrong with it.** Why purlis refused an approved
  extension's theme, or what it left out of one it draws, is shown on its row in Extensions
  instead of being dropped (#2321).
