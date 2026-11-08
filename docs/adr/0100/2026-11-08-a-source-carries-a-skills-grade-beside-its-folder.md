# A source carries a skill's grade beside its folder, and the review shows it as the publisher's

**Amends [ADR 0100](../0100-a-skill-is-installed-from-a-git-source-as-a-pinned-copy-and-never-updates-by-itself.md), 2026-11-08**, ticket #4346, by the operator's ruling of the same day. The operator's comment on #4346 (2026-11-03) held back any commit of a grade into a source until there was a ruling. The ruling is that publish may commit a grade file beside the skill's folder. §3 promised *"its grade, if the source carries one (#2781)"*, and no source carried one: [Publishing a skill, and exporting one](2026-10-31-publishing-a-skill-and-exporting-one.md) put the grades only in the request's description, which stays on the request once it merges. Spec #2762's decision 21 keeps results out of the skill's folder, and so does this file.

## Where the grade is

- **A file beside the skill's folder, never inside it**: `<parent>/<folder>.grades.json`, where `<parent>/<folder>` is the skill's folder in the source. A skill whose folder is the source's root has nowhere beside it, so it carries no grade. Nor does one where the place beside its folder is inside another skill's folder: publish writes no file there.
- **It is read at the commit the review reads**, the commit an install pins, so the grade an install showed can be read again from its pin.

## What it holds

Only what the request's description holds, plus the tree each grade measured, newest first and at most ten (`MOST_GRADES`):

```json
{
  "format": 1,
  "skill": "<name>",
  "grades": [
    { "date": "2026-11-08", "tree": "<git tree id>", "harness": "claude-code",
      "grader": "built-in", "with": 92, "without": 40, "runs": 3, "ended": "whole" }
  ]
}
```

- `tree` is the git tree id of the skill's folder as the grade measured it. For a grade of the skill's current version, publish writes the tree git gives the folder as it publishes it, in the source's own object format, so the files that stay behind (credential-shaped names, ignored files) don't make that grade read as an earlier one. For an older grade, it writes the grade's own tree. The review compares it with the tree git holds of the folder at the reviewed commit, files an install leaves out included; for an extension's source, with the tree of the files it handed.
- No chat's text, path, cost, profile or device, as for the description.

## What publish writes

- **Publish writes the file in the same commit as the skill**, and the review lists it among the files that go, under the grades.
- **A skill with no grade gets no file.** A file that a publish would leave stale is written again: the commit always holds the grades the review showed.

## What the review shows

- **One row in the summary, Grade**, from the newest grade: its pass rate with the skill and without it, the harness and the grader, and when.
- **It says whose grade it is**: *"<source>'s grade, as published — not checked by purlis"*. purlis never shows it as a grade of its own, and never puts it in the skill's Grades tab or the eval record (ADR 0105 §5).
- **It says which version it measured**: *"graded on this version"* where its `tree` is the tree of the folder at the reviewed commit, *"graded on an earlier version"* otherwise.
- **A malformed file is left out, and the review says why** in the same row: not JSON, another `format`, another skill's name, a row missing a field or holding one purlis would not write (a tree that is not a hex id, a rate outside 0 to 100, a name that is not plain), more than ten rows, or more than 64 KiB. A file reached through a link is malformed too. Nothing from a malformed file is shown.
- **A source with no file reads as it does today**: no Grade row.
- An update's review shows the grade the newer commit carries, under what changes, as the install review shows it.

## An extension's source

The SkillSource capability hands the grade beside the files: its answer to a skill's files may hold `grade`, the same object the file holds, read with the same rules. `skill-source` is still experimental, so this needs no new protocol ([ADR 0101's amendment of the same day](../0101/2026-11-08-a-skill-sources-files-answer-may-hand-its-grade.md)). An answer without it reads as before, and its skills have no Grade row.

## What was rejected

- **The merged request's description, read from the forge.** It commits nothing, but only a forge-hosted source with a sign-in could show it, anyone with write access can edit it after the merge, it is not tied to a commit, and a source pushed to directly or mirrored would show nothing.
- **The grade inside the skill's folder.** It amends decision 21, and ADR 0105 already rejected it: a grade would become part of the version it measures.
- **git notes on the source's commit.** Forges don't show them and a request doesn't carry them. Pushing a notes ref takes write access to the source, which a publish through a fork does not have.
