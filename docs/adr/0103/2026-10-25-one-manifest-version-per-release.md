# Amendment, 2026-10-25: one manifest version per release

Operator ruling, 2026-10-25: **the component parts manifest takes one version per release, not
one per pull request.** Until now each pull request that added a family raised `parts::VERSION`
and pinned the new version's digest, so every family pull request had to take the next number
and merge after the one before it.

- **`parts::VERSION` is the last version a release shipped.** A pull request never changes it.
- **What a pull request adds is `since: NEXT`**: a family, a part, a variant, a state or a
  property. `NEXT` is a marker, `VERSION + 1`, written `super::NEXT` in a family's own file and
  `parts::NEXT` in a test, and never imported.
- **Only what a release shipped is pinned.** The digest test holds the manifest as of
  `parts::VERSION` (`parts::manifest_as_of`, which leaves out everything since a later version)
  to that version's digest, so an unreleased addition changes no released theme's manifest, and
  a change to what a release shipped still fails. Unreleased additions are checked for form: no
  `since` past `NEXT`, and none before the family that holds it.
- **The release step gives the release its version**: `node tools/theme-parts-release.mjs` in
  release prep (`docs/updating.md`), beside the changelog fold, turns every `NEXT` written in
  code into `VERSION + 1`, raises `VERSION`, pins the new digest
  (`PURLIS_THEME_PARTS_PIN=1`) and writes the theme schema again. A release with nothing since
  `NEXT` keeps its version. The release workflow refuses a stable tag while a `NEXT` is left
  (`--check`); a dev build carries them.
- **What is said is honest**: `purlis theme parts` prints `version` as the last release's, and
  an unreleased addition with `since` one above it; the schema says "manifest version N, plus
  unreleased additions marked since N + 1" while there are any.

Versions 1 to 12 were taken one per pull request, before this ruling; their numbers stay as they
are.
