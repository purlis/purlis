# Four built-in skills take names that say what they do

**Amends [ADR 0099](../0099-a-skill-lives-at-a-level-a-project-skill-reaches-a-workspace-by-grant-and-the-narrowest-copy-wins.md), 2026-11-08**, operator's ruling (spec #5714, decisions Q32 and Q33), ticket #5724. It extends the renames of the [2026-11-01 amendment](2026-11-01-built-in-skills-are-purlis-name-on-every-harness.md).

## What holds now

- **Four built-in skills are renamed**, folder and `name:` together:

  | Was | Is |
  | --- | --- |
  | `purlis-compact` | `purlis-tidy-memory` (the old name collided with Claude Code's `/compact`) |
  | `purlis-working-in-a-clone` | `purlis-repo-work` |
  | `purlis-update` | `purlis-upgrade` |
  | `purlis-add-curation-action` | `purlis-curation-action` |

- **No stub keeps an old name in the bundle.** A command typed by an old name (`/purlis:purlis-compact`) gets the harness's unknown-command answer, as `/purlis:handoff` does.
- **Every old name reads as the new one where purlis reads it**: `RENAMED` in `crates/skills/src/reference.rs` is the one table. A persona's `skills:` list keeps getting the skill, by the old name bare or under either prefix. A skill run, a grade and a `skill.missed` row recorded under an old name show under the new one (`purlis_skills::name_now`), so a skill's history in Skill studio is one history.
- **`purlis persona lint` warns** of a `skills:` entry by an old name, and of a curation action whose prompt names an old name, since its chat would ask for a skill no harness has.
- **The built-in curation actions keep their ids** (`charter/compact`, `charter/add-curation-action`): an action's id is not a skill's name, and a person's pinned or remembered action stays found. Their prompts name the new skills.
