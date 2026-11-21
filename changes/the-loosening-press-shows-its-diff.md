### Changed

- **The project's policy notice shows what Accept changes.** When a commit loosens the project's
  policy, or a tightening is taken out of your own settings file, the notice lists each rule as
  loosens, tightens or changes, each mode and limit before and after, the commit, and the
  author it names (purlis cannot check who wrote it). It diffs
  against what you accepted and everything sealed since, so a commit that loosens only what an
  earlier commit tightened still shows as a loosening. Accept and Not now replace the old choices
  (M75-15, #7024).
