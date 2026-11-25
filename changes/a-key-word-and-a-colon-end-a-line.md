### Fixed

- **A save no longer refuses prose that ends in `token:` for the line below it.** The rule that
  refuses a credential assignment let the blanks after `:` or `=` run across a line break, so a
  line like "glab keeps its token:" took the next line, a shell snippet, as its value. A value
  now has to be on the keyword's own line; `TOKEN=…` and `token: …` with a literal value are
  still refused, by a save, a memory write, a session record and a dispatch brief alike
  (#7628).
