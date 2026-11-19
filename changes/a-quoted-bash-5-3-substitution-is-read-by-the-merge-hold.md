### Fixed

- **A merge inside a double-quoted bash 5.3 substitution is held.** `echo "${ gh pr merge 12; }"`
  and `x="${| gh pr merge 12; }"` run the merge on bash 5.3 and later, and the merge hold read
  neither; both are held now, as `"$( … )"` is (#7246).
