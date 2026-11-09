# The built-ins fit in 5,000 characters, and opencode lists 8,000

**Amends [ADR 0102](../0102-a-skill-router-decides-what-a-chat-sees-and-nothing-it-hides-is-out-of-reach.md), 2026-11-08**, spec #5714. It changes opencode's and Codex's rows of the [amendment of 2026-11-02](2026-11-02-a-chat-s-skill-budget-is-its-harness-s-and-the-setting-overrides-it.md).

**Who decided what.** The person ruled (spec #5714's Q37, 2026-11-08, after CI showed it): every built-in skill's description together is at most 5,000 characters, enforced by the shipped-skill check, and opencode's default budget rises to 8,000. The 1,024-character cap on one description stays. Codex's default rises to 8,000 as well, per the ruling's intent that about 3,000 of Codex's listing is left for plane skills.

## Why

The 17 built-ins' descriptions came to 10,823 characters, past opencode's and Codex's 6,000 on their own, so the router hid every plane skill on both. Codex lists skills in 8,000 characters where it does not know the window and drops what does not fit, so 5,000 for the built-ins leaves about 3,000 for the plane's own skills there.

## What changes

| Harness | Budget | Change |
|---|---|---|
| Claude Code | 60 skills, 15,000 characters | None |
| Codex | 30 skills, 8,000 characters | Was 6,000; now exactly the listing Codex holds where it does not know its window, so the default never warns |
| opencode | 30 skills, 8,000 characters | Was 6,000 |

The shipped-skill check (`crates/purlis-cli/tests/it/every_shipped_skill_passes_the_built_in_checks.rs`) fails when the built-ins' descriptions together pass 5,000 characters, and names the biggest. `docs/development.md`'s house style says about 290 characters each.

## What it leaves

- **Codex's briefing names a folder of skills once** (spec #5714's Q38, ruled by the person
  2026-11-08). Codex's budget counts the paths its briefing lists. The built-ins' `SKILL.md`
  paths, about 167 characters each where a chat's set lays them out, took 2,840 of its 8,000 and
  left under 200. Now skills at `<folder>/<name>/SKILL.md` are listed under one line naming the
  folder, the model is told to read each file by that whole path, and the budget counts the
  folder once (`purlis_skills::listing`, `budget::spent`). A skill in a folder named otherwise
  keeps its whole path on its line. A skill Codex finds itself (its own, or a repo's) still
  costs its whole path, as Codex's own list names it (`budget::ListedBy`).
- **The built-ins now leave more than 2,800 of Codex's 8,000** wherever the bundle sits
  (`the_built_ins_codex_listing_leaves_room_for_the_planes_skills`). A start is still seen by the
  read of the whole path, the same in a chat's hook and in the CI grader
  (`runs::started_by_a_read`). The CLI test that lists two plane skills on Codex, and the Skill
  studio's e2e spec, no longer raise their own `[skills.budget]`.
- **The trigger bars are not measured on the shorter descriptions.** The CI grader measures them once its keys exist.
