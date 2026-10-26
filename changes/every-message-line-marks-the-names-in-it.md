### Fixed

- **Every line purlis says marks the hidden characters in the names it quotes.** A colour
  code, a direction-changing or a zero-width character in a skill's, a persona's, a repo's or a
  piece's name, in the reason given to `worktree abandon`, or in anything else a `•`, `✓`, `!`
  or `✗` line quotes, is now written as `⟦U+XXXX⟧` instead of reaching your terminal, where it
  could hide or reorder the rest of the line. A line break inside a quoted name is marked the
  same way, so it cannot start a line that looks like purlis's own.
  purlis's own colour is unchanged (#3225).
