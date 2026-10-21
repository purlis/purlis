### Added

- **A theme exports to and imports from the W3C design-tokens format.** `purlis theme export
  <theme> --kind design-tokens` writes a theme folder as one `.tokens.json` file in the Design
  Tokens Community Group's format 2025.10, for a design tool, and `purlis theme import` reads one
  back, from purlis or from a design tool. Exporting and then importing gives the same theme,
  its component parts, grey tiers and files included. `theme.json` stays the format purlis reads
  a theme in (#2926).
