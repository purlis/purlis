# A skill that cannot be loaded is said on the chat's tab

**2026-10-22**, ticket #2771 (SK-8), amending
[persona skills and the skills list as built](2026-10-21-persona-skills-and-the-skills-list-as-built.md),
under the operator's spec #2762 (decision 41: "a skill that fails to load raises a Notice with a
fix, never a silent drop").

The chat tab's Notice of the skills a chat started without (SK-5) said only what its persona
names that is not here. **It now also says each plane skill the chat would have loaded that could
not be**: a folder that is not a whole skill (no `SKILL.md`, or one that names no `name` or
`description`), one of two folders at one level that name the same skill, or one purlis hands to
no chat because its folder or `SKILL.md` is a link or its frontmatter is refused (ADR 0063 as
amended 2026-10-22). Each is named with
its level and why, in the set's own words. A project skill whose grant does not reach the chat's
workspace is no loss to it, and is not said. The Notice is raised for a chat on no persona too,
and its Open in Skill studio opens the studio on that chat's place, where the skill is fixed in
its editor.

The live lint (`skills::lint`, SK-8) says the same of a skill as it is typed, and refuses a save
while its frontmatter is not whole.
