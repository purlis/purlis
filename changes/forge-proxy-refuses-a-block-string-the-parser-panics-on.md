### Security

- **A chat's GraphQL block string with non-ASCII whitespace can no longer abort the app.** The
  parser the forge proxy reads a document with panicked on such a line (U+3000, for one), and a
  shipped build aborts on any panic, so one chat's request ended every chat. The proxy now refuses
  such a document before the parser reads it (#7589, #7596).
