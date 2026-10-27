### Changed

- **A line break the shell does not use is marked in a command.** A command holding a lone
  carriage return, a next line (U+0085), a vertical tab, a form feed, or a line or paragraph
  separator now shows it as its mark (`⟦U+2028⟧`, for one) in the Inbox. It used to read as a
  plain space. The row says the character breaks the line there, which is why only Deny is
  offered. Where a command holds several hidden characters, the row names a line break first.
  A control, a bidi override or a zero-width character is described as one a reader does not
  see, no longer as one that looks like another. A forge login line holding any character the
  one rule hides is no longer typed, where before only control characters stopped it. The
  window now asks the core's own generated tables for which Brahmic and Arabic letters and
  viramas a joiner may stand beside, not the browser engine's (#3470).
