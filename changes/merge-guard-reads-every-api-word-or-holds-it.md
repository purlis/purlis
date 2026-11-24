### Security

- **The merge hold reads a `gh` or `glab` call by one rule, and holds what it cannot read.** A
  forge CLI takes its noun and its verb from the first words that are no flag. If the shell may
  turn a word at or before the verb into more words, or into a flag (an unquoted `$V` or `$(…)`,
  a glob, a brace, `''$V''`, `${V%/}`, a word that begins with an expansion), the call could be a
  merge, so it is held. That covers the noun, the verb, a flag, and a flag's value (`-R $REPO`;
  `-R "$REPO"` is one word and passes), and a flag before the noun that may take a value
  (`gh pr -b view $V 5`) moves the verb one word on. The same rule reads a call directly, behind
  a program no table knows (`chronic gh …`, `ssh h gh …`), and in a `for` over a forge answer.
  A word is literal only where the lexer says it has no expansion outside single quotes.
  After a literal noun and a verb that only reads, a filled-in word can no longer make a merge.
  A `gh api` call is a write when any word is a method flag, however it is quoted or attached
  (`'-XPUT'`, `-X"PUT"`, `--method=PUT`). A bare `{}` beside an endpoint behind another program
  is a filled-in word. Words a pipe or here-string hands a program in front of `gh`
  (`echo pr merge 5 | xe gh`) could complete its noun and verb, so that call is held.
  A path that names gh (`cp /opt/homebrew/bin/gh ~`, `git add crates/gh *.md`) or a package
  (`brew install gh $PKGS`) is no call. The refusal says to write the noun and the verb out, put
  the filled-in value after them, or double-quote a flag's value. The text guard is an early
  warning; under `guards.merge = enforce` the forge route is the boundary (#7423).
