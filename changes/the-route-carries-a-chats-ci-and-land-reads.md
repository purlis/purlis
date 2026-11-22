### Fixed

- **A chat's CI and landing work goes through the forge route.** A chat can now read a branch's
  rulesets and rules and a pull request's merge queue state, download a CI job's log or a run's
  artifacts, and rerun a failed run or approve one held for a fork, all on its own repo only.
  Merges, releases, tags, repository administration and every change to a rule stay refused.
