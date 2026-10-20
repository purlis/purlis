### Added

- **A chat's theme changes are a draft, drawn only in its workspace.** `purlis theme draft
  <folder>` has the app make the chat's draft in the plane's local folder, never committed, and
  `--save` hands the chat's working copy over and says what the window draws of it. A strip on the
  workspace names the draft and its chat, with Keep, Revert and Compare; ⌘⇧⌫
  (Ctrl+Shift+Backspace elsewhere) reverts at once, and closing the chat asks Keep or Revert. A
  draft that breaks a hard floor is not drawn: its last good version stays, and the chat is told
  which floor (#2332).
- **`purlis theme check` reports the hard floors** of every combination: primary text on every
  surface, the focus ring and the terminal's words, each at 3:1 as the window draws the theme.
  `--json` lists them in `hard_floors` (#2332).
- **Use in the whole project and Copy to my machine** in Settings › Theme move a workspace's
  theme to the project, and copy a project's or a workspace's to your own themes (#2332).
