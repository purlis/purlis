### Fixed

- **A guard's refusal names its real cause** (#6962). A check that ran out of time on a busy
  machine says so and to try again, instead of calling the command too long or too deeply
  nested. Creating a branch through the API is no longer called a merge: a ref written out
  literally under `refs/heads/` is allowed, a tag is held as a release, and a ref purlis cannot
  read is refused with "can't tell what this ref writes; write it out literally". Committing a
  file through the API to a branch of its own is allowed, and to the default branch (or with no
  branch named) is refused, unattended, as the push to the default branch it is, naming
  `purlis worktree push`. And a `curl --variable` placeholder (`{{NAME}}`, named by a
  `--variable` on the same line) is a name, not a credential, to every guard that reads for one
  and to `purlis save`.
