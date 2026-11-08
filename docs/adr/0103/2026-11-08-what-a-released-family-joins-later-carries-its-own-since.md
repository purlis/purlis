# Amendment, 2026-11-08 (#3226): what a released family or part joins later carries its own since

The amendment of 2026-10-25 gives every addition `since: NEXT`, and a family, a part, a variant,
a state and a property each carries one. The lists that join them do not: a family's `states`
and a part's `properties`. A pull request that gave a released family a state the manifest
already had (the popover gaining `focused`), or a released part a property it already had (the
button's label gaining `padding`), had nothing to mark as unreleased. The manifest as of the last
release took the addition in, so the released digest changed and the pull request could not land;
had it been let through, the addition would have shipped under a released version.

- **A family dates what its lists gained after it shipped.** `Family::later` holds each such
  addition, a state of the family's (`Joined::State`) or a property of one of its parts
  (`Joined::Property`), with the version it came in, written `since: super::NEXT` as any other
  addition. The list itself still names it, as ever, so the schema, the key grammar and the
  check read one list and never this one.
- **The manifest as of an earlier version leaves it out.** `parts::manifest_as_of(v)` lists a
  state or a property in a family's or a part's list only where `v` is at or past both its own
  `since` and the one `later` gives it, so what a release shipped keeps its digest, and
  `purlis theme parts` and the schema say nothing new of a released item until the next release.
- **The release step numbers it like any other.** `NEXT` in a `later` entry is code, so
  `node tools/theme-parts-release.mjs` replaces it, and `--check` and the literal check
  (`since: <VERSION + 1>`) both find one left. A test holds each entry to its lists: it names a
  state its family has or a property its part has, once, after what holds it, and no later than
  `NEXT`.

Nothing released moves: every family's `later` is empty today.
