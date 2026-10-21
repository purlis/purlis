### Changed

- **Imported themes keep their own quiet and disabled greys.** `purlis theme import` maps a VS
  Code theme's line numbers (`editorLineNumber.foreground`) to `text.quiet` and its disabled
  words (`disabledForeground`) to `text.faint`, and its muted words take the louder of the two.
  A base16 or base24 scheme's invisibles (`base03`) are its faint, walked to the 3:1 mark floor.
  No tier is drawn louder than the one above it: a tier the source has no grey for, or whose
  grey would read louder, follows the tier above it, as faint follows quiet and quiet follows
  muted (#2941).
