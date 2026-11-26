### Fixed

- **An auto-merge is no longer missed in three more spellings (#7620).** `gh pr merge 12 -m --auto`
  is refused as an auto-merge (`-m` is gh's `--merge`, not a flag that takes a value), a REST merge
  that sets the key to `1`, `"t"`, `"yes"`, `"on"` or any other value GitLab reads as true is
  refused, and so is a merge-train entry that sets `when_pipeline_succeeds`.
