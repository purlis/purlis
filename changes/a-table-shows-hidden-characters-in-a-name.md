### Security

- **A name that hides or reorders its own characters can no longer disguise a table row.** Every
  table purlis prints in the terminal, from `purlis skill list` to `purlis repo status`, shows a
  name's control, bidi and zero-width characters as `⟦U+XXXX⟧`, the way the install review
  does, and lines its columns up on what it shows. purlis's own colours still show (#3150).
