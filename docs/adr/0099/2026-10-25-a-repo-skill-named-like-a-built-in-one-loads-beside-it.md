# A repo skill named like a built-in one loads beside it, as #3027 built it

**Amends [ADR 0099](../0099-a-skill-lives-at-a-level-a-project-skill-reaches-a-workspace-by-grant-and-the-narrowest-copy-wins.md), 2026-10-25**, ticket #3027 (SK-37), spec #2762 decision 41. It corrects one line of the [2026-10-21 amendment](2026-10-21-harness-and-repo-skills-as-built.md): "a built-in skill never loses … the other is listed hidden by purlis's built-in skill of this name".

## What was wrong

- purlis never turns off a skill named like a built-in one, on any harness. A switch that goes by the name alone would reach purlis's own copy. So a harness reads the other copy where it lies, beside purlis's. On Codex and opencode both copies have the same name, and the model may pick the repo's. Calling the other copy "hidden" was not true.

## What holds now

- **A built-in skill still never loses.** purlis's copy is handed on every route. A clone cannot take purlis's own `secrets` away from a chat.
- **The other copy is a clash, and both load** (`Outcome::BothLoad`), on every route, for a repo copy and a harness copy alike. `SkillSet::with_outside` lists it as on, by `Route::Own`, and `SkillSet::beside_a_built_in` names these clashes. `Reason::HiddenByBuiltIn` is gone.
- **The Skill studio marks the built-in skill** the same way it marks a plane skill that clashes. The other copy's row is no longer left out, and the built-in lints warn that its name is a built-in one's (`Lint::BuiltInName`). The lint checks the names the studio's set was worked out with.
- **A chat started in that clone says so on its tab** (`NotHere::beside_a_built_in`), in the Notice that names the skills a chat starts without.
