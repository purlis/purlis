### Fixed

- **A text box, a select and a combobox have a visible edge in Purlis**, so an empty box can be
  found on the window and in a panel or dialog. The edge holds 3:1 against what the box stands on
  (WCAG 2.2, 1.4.11). Themes can set it with the new `border.input` token; a theme that leaves it
  out draws its boxes as before.
