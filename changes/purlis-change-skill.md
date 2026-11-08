### Added

- **A built-in skill for one change across several repos.** When you ask a purlis chat to keep
  work that spans several repos together, such as an API bump and the repos that use it, the new
  `purlis-change` skill guides it through `purlis change`: create the change, add each repo and
  what must land before it, push every branch with linked pull requests, land them one at a time
  in order, and roll the whole change back. It says that landing merges code and is yours to ask
  for, and that a pull request merged in the browser holds back the repos waiting on it.
