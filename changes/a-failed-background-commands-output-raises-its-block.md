### Fixed

- **A failed background command's output, read back by file, raises its Block.** Reading a
  background command's output file with the file tool, `cat` or `tail` raised nothing, even when
  the command had failed on a refusal. Where Claude Code's own record of the task says it
  failed, what was read is now that command's own error, and its Block comes up with what it
  names (#2203).
- **More readers that can start a program are read as running one**: `rg --pre`, `sort
  --compress-program`, `less` and `more`, and `git` given configuration on its line, an external
  diff, or `git grep -O` (#2203).
