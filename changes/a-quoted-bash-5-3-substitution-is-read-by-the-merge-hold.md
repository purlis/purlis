### Fixed

- **A merge inside a double-quoted bash 5.3 substitution is held.** `echo "${ gh pr merge 12; }"`
  and `x="${| gh pr merge 12; }"` run the merge on bash 5.3 and later, and the merge hold read
  neither; both are held now, as `"$( … )"` is (#7246).
- **The merge hold reads a bash 5.3 substitution to the `}` bash closes it at, and no regex is
  one.** A `}` after `>&`, `>|`, `a=(` or a `||` in `[[ … ]]` closed the substitution early and
  left the rest unread; it no longer does. A `${` in single quotes or escaped, as in
  `grep -E '\${[A-Z_]+}' f`, is text and is not refused (#7250).
