### Security

- **The merge hold reads every word of a `gh api` or `glab api` call, and holds the ones it can't.**
  The round before listed spellings, and each list left one: a quoted endpoint followed by an
  unquoted substitution, `$''` and `$""` prefixes, `$@` and arrays in double quotes, zsh's `${=F}`
  and `$=F`, a glob, a bracket inside a quoted substitution, a program found by its spelling
  rather than its place (`>gh gh api …`, `env -u gh gh api …`), a flag before `api`
  (`gh -X PUT api …`), a function body, zsh's `noglob` and `-`, `xargs` and `find -exec` into
  `gh`, an `env -S` string, a shell alias for `gh`, and an inner shell's string that the outer
  shell fills in. A word is now let through only when it has no expansion, glob or brace outside
  single quotes, or is one literal start and one double-quoted span of a plain `$name` or `$(…)`,
  and the call is found where the reader finds the program. `&>` is one redirection, not a `&`.
  A line of thousands of references reads in one pass, not one scan of the line per reference.
  The refusal says to write the endpoint out, or to use one double-quoted word with a literal
  start (#7423).
