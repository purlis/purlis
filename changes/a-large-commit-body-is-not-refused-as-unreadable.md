### Fixed

- **A commit message or pull request body with a `)` or a quote in it is no longer refused as
  unreadable.** A chat's `git commit -m "$(cat <<'EOF' … EOF)"` whose message said `1) first` or
  held a lone `"` was held as a merge purlis could not read, and on a slow machine ran out of its
  time first (#7131). A literal `gh pr merge` on a line of such a message is still held.
