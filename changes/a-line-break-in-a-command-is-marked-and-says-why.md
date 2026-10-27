### Changed

- **A line break the shell does not use is marked in a command.** A command holding a next
  line (U+0085), a vertical tab, a form feed, or a line or paragraph separator now shows it as
  `⟦U+2028⟧` (or that character's code) in the Inbox. It used to read as a plain space. The row
  says the character breaks the line there, which is why only Deny is offered. A forge login
  line holding any character the one rule hides is no longer typed, where before only control
  characters stopped it. The window now asks the core's own generated table for which Brahmic
  and Arabic letters a joiner may stand between, not the browser engine's (#3470).
