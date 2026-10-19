### Added

- **The theme check holds more of what the window draws.** Each syntax colour is held on the
  editor's current line, the window's secondary, muted, accent and state words on a theme's
  text selection, and a line of words has to fit the control or strip it is drawn in, in both
  densities: a theme whose text size and line height make a line taller than its box is told the
  sizes (#2522).
- **A chart draws each point in its own colour**, the theme's chart palette in turn (#2522).

### Changed

- **Purlis Classic light's code properties and attributes are a shade darker** (`#1283ff`, was
  `#218bff`), so they read on the editor's current line (#2522).
