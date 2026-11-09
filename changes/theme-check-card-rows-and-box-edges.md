### Fixed

- **`purlis theme check` says when a theme's side-view cards blend into their panel or a text
  box loses its edge**, even when the theme sets no component parts. A card row has to stand a
  step from its panel. A text box's edge has to hold 3:1 on the window, a panel and a dialog.
  Code has to read at 4.5:1 on the editor's own background. The check only reports these.
  Purlis draws the theme as written.
- **Text boxes have a visible edge in Purlis Classic**, at 3:1 or more on every ground they
  stand on.
