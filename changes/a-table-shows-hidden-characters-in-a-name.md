### Security

- **A name that hides or reorders its own characters can no longer disguise a row.** The tables
  of `purlis skill list`, `purlis skill runs`, `purlis recall`, `purlis status`, `purlis worktree
  list` and `purlis worktree history` show a name's control, bidi and zero-width characters as
  `⟦U+XXXX⟧`, the way the install review does, in every column, the last one too, and line their
  columns up on what they show. purlis's own colours still show (#3150).
