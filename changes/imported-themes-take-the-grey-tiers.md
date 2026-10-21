### Changed

- **Imported themes keep their own quiet and disabled greys.** `purlis theme import` maps a VS
  Code theme's line numbers (`editorLineNumber.foreground`) to `text.quiet` and its disabled
  words (`disabledForeground`) to `text.faint`; its muted words take the line numbers' grey
  first. A base16 or base24 scheme's invisibles (`base03`) are its faint, walked to the 3:1 mark
  floor. A tier the source has no grey for follows the imported muted, as before (#2941).
