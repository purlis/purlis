# The UI-building guide leaves the bundle for a plane skill

**Amends [ADR 0099](../0099-a-skill-lives-at-a-level-a-project-skill-reaches-a-workspace-by-grant-and-the-narrowest-copy-wins.md), 2026-11-08**, the person's ruling on spec #5714 (Q35), ticket #5725. It follows the [2026-11-01 amendment](2026-11-01-built-in-skills-are-purlis-name-on-every-harness.md), which named the guide `purlis-building-ui`.

## What holds now

- **purlis no longer ships `purlis-building-ui`.** It applies only to a chat developing purlis
  itself, yet every person's chats listed it. Its facts are
  [`docs/building-ui.md`](../../building-ui.md) in the purlis repository, held to the code by
  `app/src/ui/buildingDoc.test.ts`, the test that held the skill. The repository's `AGENTS.md`
  points to it.
- **A plane that develops purlis keeps a project skill for it**, `building-purlis-ui`, named
  without the reserved prefix. Its body is the trigger, "in a purlis/purlis checkout, follow
  `docs/building-ui.md`", and that plane's own workflow. It is made through Skill studio, granted
  to every workspace and pinned, before the bundle drops the built-in.

## Old names keep working

- **Every name the guide had reads as the plane skill `building-purlis-ui`**: `purlis-building-ui`
  and `purlis-building-purlis-ui`, bare or under `purlis:` or `charter:`, and `purlis:building-ui`
  and `purlis:building-purlis-ui`. `Reference::parse` reads them as `Reference::Named`, so a
  persona's `skills:` list reaches the plane skill where the project has it, `skill_load` finds it
  by an old name, and a run recorded under an old name counts as the plane skill's.
- **The persona lint says where it went**: an entry by an old name is a warning naming
  `building-purlis-ui` to write in its place. Where the project has no such skill, the entry is
  also one its chats start without, as any other missing name is.
- **No stub skill keeps an old name**, and `/purlis:purlis-building-ui` typed in Claude Code gets
  its unknown-command answer, as the 2026-11-01 amendment accepted for every rename.
