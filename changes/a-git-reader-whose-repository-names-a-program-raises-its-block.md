### Fixed

- **A git reader whose repository's config names a program raises its Block.** `git log`,
  `git status` and the other reading subcommands, run in a repository whose own config names an
  fsmonitor hook, a diff or textconv driver, a filter, a signature program, an index hook or a
  promisor remote, are read as running a program, so a refusal that program met comes up as a
  Block (#2212).
