### Changed

- **A theme made on one platform opens on every other.** A theme folder's name and every name
  inside it hold none of `< > : " / \ | ? *` nor a control character: `purlis theme import`,
  `export` and the theme folder reader each refuse such a name and say why in one sentence
  (#2671).
- **Every theme file is read from the folder purlis opened.** `purlis theme check <folder>`,
  your own `theme.json` and `purlis theme export <level id>` open the folder once, refusing a
  link, and read every file from it (#2604, #2670).
