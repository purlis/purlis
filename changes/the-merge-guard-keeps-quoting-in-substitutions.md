### Fixed

- **The merge guard no longer refuses a read-only line over a quoted `|` in a substitution or
  in `${…}`.** It no longer reads `.[]` in `done < <(jq -r 'sort_by(.iid) | .[]' f.json)` as a
  program, and `f=${t%%|*}` is now one assignment, not a pipe into `*}`. A merge inside a
  substitution, such as `< <(gh pr merge 1)` or `$(glab mr merge 2)`, is still held (#7709).
