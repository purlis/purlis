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
  word. A redirection is read once, exactly as the shell reads it: the lexer keeps whether a word
  was quoted and whether it is glued to the one before, so `2>f` is a descriptor and `nice -n 5 >f`
  has a value, a quoted `'<'` is a word, and a redirection before the program, after `-R o/r`, or
  after a keyword (`then`, `do`, `!`) is taken out wherever it stands (`nice -n 5 </dev/null 2>&1
  gh …`, `env -u '<' gh >/dev/null …`, `sudo -u a nice -n 5 >f {fd}>g gh …`). Where the words in
  front of `gh` or `glab` cannot be placed (a wrapper flag that no table lists, a wrapper that
  none knows), the call is held, and the refusal says to put `gh` first in the command or run it
  without the wrapper. `watch` runs its words as a shell string and is read as one. An `alias
  set` with a redirection or stdin, zsh's `env =gh` and `sudo =gh`, a program word the shell
  fills in before `api`, and an escaped `\$` inside backticks (a quoted backtick opens nothing,
  and `\\` before one escapes the backslash) are held (#7423). A run of values and descriptors
  in front of a call is read in time proportional to its length. A forge call behind any program
  that is no known wrapper (`taskset 1`, `chronic`, `ssh h`, `fd -x`) is held when it merges or
  uses `api`, whatever the program is, and `timeout` takes its first operand as the duration
  however it is spelled (`.5`, `1e1`, `0x10`). zsh's clobber operators (`>!`, `>>|`, `&>!`, …), `A+=1`
  assignments, `repeat`, short `for` and `if`, dash's `&>` in an inner `sh -c`, `env -S`'s `\_`
  separator and a bare `"$@"` as the program are read as the shell reads them. The no-value flags
  `timeout --foreground`, `sudo --login`, `env --ignore-environment`, `watch --no-title` and
  `nice -10` no longer hold a read.
