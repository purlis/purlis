### Security

- **The merge hold reads every word of a `gh api` or `glab api` call, and holds the ones it can't.**
  The round before listed spellings, and each list left one: a quoted endpoint followed by an
  unquoted substitution, `$''` and `$""` prefixes, `$@` and arrays in double quotes, zsh's `${=F}`
  and `$=F`, a glob, a bracket inside a quoted substitution, a program found by its spelling
  rather than its place (`>gh gh api …`, `env -u gh gh api …`), a flag before `api`
  (`gh -X PUT api …`), a function body, zsh's `noglob` and `-`, `xargs` and `find -exec` into
  `gh`, an `env -S` string, a shell alias for `gh`, and an inner shell's string that the outer
  shell fills in. A word is now let through only when it has no expansion outside single quotes,
  or is one literal start and one double-quoted span of a plain `$name` or `$(…)`, and the call
  is found where the reader finds the program. A glob or a brace is let through after a literal
  start that is no `-` (`pulls?state=open`, `pulls/{1,2}`), since every word it makes begins with
  that start, and held when it begins the word (`*`, `?XPUT`, `{-X,PUT}`). `&>` is one redirection, not a `&`.
  A redirection with a descriptor (`gh 2>/dev/null api …`, `gh pr 2>/dev/null merge 5`), zsh's
  `-` after `noglob` or an assignment, a quoted `'>'` that is an argument, a `'` inside double
  quotes, and a flag before the noun that may take a value (`gh -b x pr merge 5`) no longer hide
  the call. A line of tens of thousands of `$N` references reads in linear time. The text guard
  is an early warning, not the boundary: the forge route is, under `guards.merge = enforce`.
  The refusal says to write the endpoint out, or to use one double-quoted word with a literal
  start; for a held glob it names the character (`?`, `*` or `[`) and says to quote the whole
  word. A redirection before the program or after `-R o/r`, a wrapper's numeric or quoted option
  value (`nice -n 5 </dev/null 2>&1 gh …`, `env -u '<' gh >/dev/null …`), several option values, an
  `alias set` with a redirection or stdin, zsh's `env =gh` and `sudo =gh`, a program word the shell
  fills in before `api`, and an escaped `\$` inside backticks are read, each way the lexer's
  split allows, and held (#7423).
