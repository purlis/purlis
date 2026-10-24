# Lifecycle states, owners, a skill's history and the uses graph, as #2782 built them

**2026-10-24**, ticket #2782 (SK-19), amending
[ADR 0099](../0099-a-skill-lives-at-a-level-a-project-skill-reaches-a-workspace-by-grant-and-the-narrowest-copy-wins.md)
§1 and §4 under the operator's spec #2762 (decisions 23, 57, 62 and 63).

## Where it is kept

The lifecycle state and the owners are kept beside the skill's folder, in `<name>.toml` with its
grant, as §1 said they would be: `lifecycle` (`"deprecated"` or `"archived"`, absent for a skill
in use), `replacement` for a deprecated one, and `owners`. The file is read following no link, at
most 64 KiB, as TOML, and only those keys are taken from it, each checked against an allow-list.
One writer (`purlis_skills::grant::edit_beside`) writes the grant, the state and the owners, and
keeps every other key. Each change from the tab or `purlis skill` is one commit of that file.

## What each state does

- **Deprecated**: the skill still reaches every chat it reached. The set marks it
  (`Effective::deprecated`, with its replacement), so a router can list it after the rest and name
  its replacement (ADR 0102 §2). Lowering it is the router's, SK-15 (#2778); until it lands, the
  listing a harness is handed is unchanged, and the studio lists a deprecated skill after the
  others.
- **Archived**: the skill never loads. §4's "archived" reason is now real: the set leaves it out
  with "archived", at every plane level, and **an archived copy takes no name**, so a wider copy of
  the same name reaches the chat instead, and the archive says so ("the project copy now loads"). It stays in the project and in its history, and is
  restored by making it active again.
- **A state purlis cannot read** (an unknown word, a file that is not TOML) is read as in use: the
  skill keeps working, and its tab says why. Failing closed here would hide a skill over a typo.
- **Unused for 60 days**: a skill in use whose copy no chat ran for 60 days, counted from its last
  run or from its first commit, gets a suggestion on its tab to deprecate it. Nothing is changed
  on its own.

## Owners are informational

Decision 63's review of a teammate's change to an owned skill on an organisation's project needs
the policy layer (SD-14, #697), so it follows that ticket. Until then owners are shown on the tab
and in `purlis skill owners`, and change nothing.

## History

A version is a commit that changed the skill's folder, at most 100, newest first. A commit that
changed only the file beside it (a deprecation, owners, a grant) is no version, and Revert never
offers it. **Runs are matched to versions by content, not by time**: each run's start records the
tree git's `HEAD` holds of the copy's folder (`tree` on the start row, SK-20's row), or
`uncommitted` where the folder held changes not committed. A run counts on the version whose folder
has that tree, so a teammate who pulled a version later is counted on the version they ran; an
uncommitted run is counted apart. The totals are SK-20's one reading of the log
(`skilluse::stats`, `By::Version`).

**Revert to this** restores the folder as that commit had it, in one commit, and leaves the file
beside it alone: the grant, the state and the owners change by their own controls, so a revert
never re-archives or un-grants a skill. It is refused while the folder holds changes not committed,
for a version whose `SKILL.md` is not a whole skill of the same name, and **for a version that holds
a link or another repository**, read from git's tree before anything is restored, so no link is
ever brought back. Its confirm shows `widening_said` where the version's frontmatter would widen a
chat; SK-8 still leaves those keys out when purlis hands the skill over. A version's diff is cut at
256 KiB, and each character that draws as nothing is shown by its code point and said. Each
version has an empty grade slot for SK-16.

## The uses graph

A skill's text naming another (`skills:<name>`, `purlis:<name>`) is an edge. It is read by the one
reader of references (`purlis_skills::uses`), which the broken-reference lint now reads by too, so a
reference the graph marks broken is the one the editor warns of. At most 2,000 skills and 200
references a skill are read. A nested run was already recorded with its parent (SK-3); the tab
now shows each run's parent skill and the skills that ran inside it.
