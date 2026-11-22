### Fixed

- **A chat may merge through its forge route where you set `guards.merge` to `log` or `off`.**
  The merge guard already let a merge through under those settings, but the route a chat's `gh`,
  `glab` and git use to reach its forge still refused every merge. Now `gh pr merge` (with
  `--auto` and the merge queue), `glab mr merge`, GitLab's merge when the pipeline succeeds and a
  merge by the API go through for a repo your chat's own run recorded. `log` writes one line to
  the chat's enforcement log, naming the chat and the repo and never the request; under
  `enforce` a merge is still asked in the window. A tag, a release, a ruleset, the repository's
  administration, another repo and a branch merge stay refused, an administrator's lock still
  wins, and a policy purlis cannot read is `enforce`.
