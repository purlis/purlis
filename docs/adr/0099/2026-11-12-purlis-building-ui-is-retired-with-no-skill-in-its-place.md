# purlis-building-ui is retired, with no skill in its place

**Amends [ADR 0099](../0099-a-skill-lives-at-a-level-a-project-skill-reaches-a-workspace-by-grant-and-the-narrowest-copy-wins.md), 2026-11-12**, by the person's ruling of 2026-11-12, ticket #5725. It follows the [2026-11-01 amendment](2026-11-01-built-in-skills-are-purlis-name-on-every-harness.md), which named the guide `purlis-building-ui`.

## The ruling

Spec #5714 (Q35) planned to move the window-building guide out of the bundle and into a project skill of the plane, `building-purlis-ui`, and have every name the built-in had read as that plane skill. Pull request #5826 built that plan and never merged. On 2026-11-12 the person ruled that the skill isn't used any more and should be removed, and that no plane will keep a `building-purlis-ui` skill. **The skill is retired, not moved.**

## What holds now

- **purlis no longer ships `purlis-building-ui`.** Its facts are [`docs/building-ui.md`](../../building-ui.md) in the purlis repository, held to the code by `app/src/ui/buildingDoc.test.ts`, the test that held the skill. The repository's `AGENTS.md` points to the doc. No skill, built-in or plane, takes its place.
- **Every name it had still reads as it**: `purlis-building-ui` bare, under `purlis:` or `charter:`, and `purlis:building-ui`, and `purlis-building-purlis-ui` and its forms (`RENAMED`). `purlis_skills::RETIRED` lists it, with where its facts went, and `Reference::retired` reads an entry as a retired skill. So:
  - **the persona lint says it was removed**, with where its guide is, and does not also say that its chats start without it;
  - **`skill_load` says it was removed**, with where its guide is, rather than that the chat has no skill of that name;
  - **a run recorded under any of its names keeps the name `purlis-building-ui`** (`run_name`), so its history stays where it was.
- **No stub skill keeps the name.** `/purlis:purlis-building-ui` typed in Claude Code gets its unknown-command answer, as the 2026-11-01 amendment accepted for every rename.
- A later retirement is one more row of `RETIRED`.

## Rejected

- **Read every old name as a plane skill `building-purlis-ui`** (#5826, spec #5714 Q35). The person ruled there will be no such skill, so a name read as it would reach nothing, and the lint would send a persona to write a name no plane has.
- **Leave the old names to name nothing.** A persona naming the skill would then be told that purlis has no built-in of that name, as for a typo, with no word of where its guide went.
