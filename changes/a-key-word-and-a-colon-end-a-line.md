### Fixed

- **A save no longer refuses prose that ends in `token:` for the line below it.** The rule that
  refuses a credential assignment let the blanks after `:` or `=` run across a line break, so a
  line like "a tool keeps its token:" took the next line, a shell snippet, as its value. The
  next line is now read as the value only when it holds one secret-like word alone, so
  `Password:` with the secret on the line below is still refused, as are `TOKEN=…` and
  `token: …` with a literal value, by a save, a memory write, a session record and a dispatch
  brief alike (#7628).
