### Fixed

- **The merge guard no longer stops calls that merge nothing.** Playing or retrying a CI job
  through `glab api`, with the job's id written out or a variable the same line binds to digits
  (`for j in 101 102; do ...`), and editing a merge request's
  title or description with `--input` pass; a merge, a ref write to the default branch and
  anything purlis cannot read, a variable in a writing call's path that the line does not bind to
  digits included, are still held. A refusal now says which word or endpoint it took
  for a merge, and where a ref write is held it names `purlis worktree push` as the way to push a
  branch.
