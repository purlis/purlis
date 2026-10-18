### Added

- **`purlis theme import` and `purlis theme export`.** `import <file>` makes a theme folder from
  a VS Code colour theme, a base16 or base24 scheme, an iTerm2 colour preset, a Ghostty theme or
  a theme folder's zip, through a fixed mapping table per kind. It lists every key of the source
  no token takes, shows the new theme's check, and never replaces a theme folder without
  `--force`. `export <theme folder>` writes the folder as a zip that `import`, or unzip, gives
  back file for file. `purlis docs show themes` has the tables (#2334).
