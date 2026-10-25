### Changed

- **The theme parts manifest's version moves once per release.** `purlis theme parts` prints
  `version` as the last release's manifest version; a family, part, variant, state or property
  added since then is listed with `since` one above it, and the schema says "plus unreleased
  additions" while there are any. What a release shipped never changes (ADR 0103, amended
  2026-10-25).
