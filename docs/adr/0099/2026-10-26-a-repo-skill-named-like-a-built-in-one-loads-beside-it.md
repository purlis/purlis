# A repo skill named like a built-in one loads beside it, as #3027 built it

**Amends [ADR 0099](../0099-a-skill-lives-at-a-level-a-project-skill-reaches-a-workspace-by-grant-and-the-narrowest-copy-wins.md), 2026-10-26**, ticket #3027 (SK-37), spec #2762 decision 41. It corrects one line of the [2026-10-21 amendment](2026-10-21-harness-and-repo-skills-as-built.md), "a built-in skill never loses … the other is listed hidden by purlis's built-in skill of this name", for the routes that know built-in skills by their bare name. The [2026-10-26 opencode amendment](2026-10-26-opencode-gets-the-built-in-skills-under-their-own-names.md) covers the route that names them `purlis-<name>`.

## What was wrong

- purlis never turns off a skill named like a built-in one, on any harness. A switch that goes by the name alone would reach purlis's own copy. So a harness reads the other copy where it lies, beside purlis's, and the model may pick it. Calling the other copy "hidden" was not true.

## What holds now

- **A built-in skill still never loses.** purlis's copy is handed on every route. A clone cannot take purlis's own `secrets` away from a chat.
- **On Claude Code and Codex, the other copy is a clash, and both load** (`Outcome::BothLoad`), for a repo copy and a harness copy alike. `SkillSet::with_outside` lists it as on, by `Route::Own`. `Reason::HiddenByBuiltIn` is gone. On opencode a copy of `purlis-<name>` or `purlis-skill-index` stays an `Outcome::EitherOne` clash, and a bare `secrets` there is another skill.
- **`SkillSet::beside_a_built_in`** names every clash with a built-in skill, of either outcome.
- **The Skill studio marks the built-in skill** the same way it marks a plane skill that clashes. The other copy's row is not left out. The built-in lints warn that its name is a built-in one's (`Lint::BuiltInName`), checked against the names each program knows purlis's skills by (`Route::built_in_names`). A plugin's skill, known by its plugin's name, is never warned of.
- **A chat started in that clone says so on its tab** (`NotHere::beside_a_built_in`), in the Notice that names the skills a chat starts without.
