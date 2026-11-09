### Changed

- **Four built-in skills have names that say what they do.** `purlis-compact` is now
  `purlis-tidy-memory` (it no longer looks like Claude Code's `/compact`),
  `purlis-working-in-a-clone` is `purlis-repo-work`, `purlis-update` is `purlis-upgrade`, and
  `purlis-add-curation-action` is `purlis-curation-action`. A persona that lists an old name
  still gets the skill, and the runs, grades and misses recorded under an old name show under
  the new one in Skill studio. `purlis persona lint` points out an old name in a persona's
  skills list or curation action. A command typed by an old name, such as
  `/purlis:purlis-compact`, no longer works: type the new one (#5724).
