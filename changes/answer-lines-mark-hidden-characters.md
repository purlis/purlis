### Security

- **A command's answer marks the hidden characters in the names it prints.** `persona default`,
  `persona show`, `persona dedupe`, `persona log` and `persona optimize` show a control, bidi or
  zero-width character in a name, a title or a path read from a file as `⟦U+XXXX⟧`, as their
  other lines already did. An ordinary name prints as before, and a persona's charter is printed
  as it is (#3287).
