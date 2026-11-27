### Security

- **A merge or a publish behind a variable a substitution sets is held.** `A=$(true) gh pr merge
  12` and its backtick spelling passed the merge hold and the release floor, as did
  `gh release create`, `git tag` and `git push --tags` behind the same prefix: the floor read
  the substitution's own command as the program. It now reads each command's own words too.

### Fixed

- **A chat watching CI with `gh api` is no longer held as a merge.** A read-only call such as
  `id=$(gh api "repos/o/r/actions/runs?head_sha=$H" --jq … | head -1)` was refused as a merge
  purlis could not read. A merge through the API is still held wherever it stands (#7092).
