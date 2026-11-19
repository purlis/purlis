### Fixed

- **The merge guard no longer stops calls that merge nothing.** Playing or retrying a CI job
  through `glab api`, with the job's id written out or a variable on a line of exactly
  `J=101; glab api ...` or `for j in 101 102; do glab api ...; done`, and editing a request's
  text, labels, assignees, reviewers, milestone or state through the API (with fields, a JSON
  file, or a quoted heredoc on stdin) pass; a merge, a ref write to the default branch, an
  edit that sets a target branch or a merge setting, a body purlis cannot read (a relative file
  in a line that changes directory included) and a variable in a writing call's path on any
  other line are still held. A refusal now says which word or endpoint it took
  for a merge, and where a ref write is held it names `purlis worktree push` as the way to push a
  branch.
