# A name clash at install is the person's to settle

**Amends [ADR 0100](../0100-a-skill-is-installed-from-a-git-source-as-a-pinned-copy-and-never-updates-by-itself.md), 2026-11-01**, ticket #4530, the operator's ruling of that day. Its details on the prototype (`prototype/clash-review`, variant A) reached this work relayed by the chat that asked for it. §2 copied a skill "under its own name", and a name already taken was refused, or left out of a set with no choice.

## The ruling

- **The install review stops on a clash.** A clash is a skill named for install whose name the level it lands in already holds, from any source or made there, or a built-in skill's name.
- **A copy at another level does not stop it.** The narrowest-copy rule (ADR 0099) decides as it always does, and the review says so in one line.
- **Two choices:** skip it, or install it under a new name. `<source>-<name>` is filled in and can be edited. `<source>` is the source's label.
- **A new name changes only the `name:` line and the folder.** Prose references are never rewritten, in the skill or anywhere else.
- **The record keeps where it came from.** The entry in `skills.lock` keeps the source and the pinned commit as any entry does, and adds `source_name`, the name at the source. An update finds the skill by that name and keeps the new one. The pinned-copy check ignores the name line.
- **The review warns in one line** where a skill being installed refers to a name that is another source's here.
- **A skill's source shows as a label wherever a person sees a skill listed:** `grilling · mattpocock`. The label never reaches a chat.

No alias logic and no per-harness naming.

## As built

- **`change::with_name`, not the rename verb.** The verb renames a skill already at a level, which a clash forbids, and it rewrites references across the plane, which this ruling forbids. Both share `with_name`. The operator accepted this.
- **The record is a new optional key in `skills.lock`,** not a file beside the skill. The operator accepted this too. `docs/plane-format.md` has the key.
- **`there` now means installed from this source at this level already,** under either name. A name held any other way is a clash.
- **Skipping reviews again without the skipped skill,** so what it alone would have brought along is not installed.

## The measurement

A renamed sibling, referred to by its old name in prose, was found 40 of 40 times on Claude Code and 5 of 5 on opencode. Codex was not run.

## Not settled

`purlis skill rename` on an installed skill does not move its entry in `skills.lock`. That was so before this ruling.
