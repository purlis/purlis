### Added

- **A theme can colour code, markdown, the selection, the caret, placeholders, scrollbars, charts
  and the window's background.** New tokens: `syntax.*` for the light editor's code, each role
  with italic and bold; `markdown.*` for links, code, code blocks, quotes and headings in a
  markdown render; `selection.*`, `caret`, `placeholder` and `scrollbar.*` for what the platform
  draws until a theme says; `chart.1`–`chart.8`; `window.background`; and the terminal's
  selected text, unfocused selection, scrollbar, cursor style and blink, line height and bold in
  the bright colours. The editor no longer borrows the terminal's colours for code. Purlis
  Classic sets each to what it drew before (#2327).
