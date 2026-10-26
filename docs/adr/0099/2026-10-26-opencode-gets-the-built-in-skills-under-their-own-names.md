# opencode gets the built-in skills under their own names

**Amends [ADR 0099](../0099-a-skill-lives-at-a-level-a-project-skill-reaches-a-workspace-by-grant-and-the-narrowest-copy-wins.md), 2026-10-26.** §3 says a built-in skill's name is never taken, and the 2026-10-21 amendment (harness and repo skills, as built) says a repo or harness copy of a built-in skill's name is listed "hidden by purlis's built-in skill of this name". This records why opencode needs distinct names, and what it now gets.

## Why opencode needs distinct names

- Claude Code loads the built-in skills as purlis's plugin's, `purlis:<name>`, so no other skill can have that name there. Codex is briefed on each skill with the path to its `SKILL.md`, and keeps two skills of one name apart by their paths.
- opencode reads every skill by the bare `name` in its `SKILL.md`. It reads the folder purlis hands it beside the folders it finds itself: a clone's `.claude/skills`, `.agents/skills` and `.opencode/skill(s)`, and the person's own. Two skills of one name are one entry to it, and which one it keeps depends on which file it finishes reading last (measured on 1.18.33: 4 of 8 runs each way). purlis cannot settle that by leaving the other copy out, because opencode's own switch, `permission.skill`, also goes by the bare name.
- So on opencode's route the built-in skills come under names of their own, `purlis-<name>`, which mirrors Claude Code's `purlis:` namespace. A clone's `secrets` and purlis's `purlis-secrets` are then two skills, and each is reached by its own name.

## What opencode gets

- **The name belongs to the route.** `purlis_skills::Route::names_built_ins_its_own` is true for `Route::Config`, opencode's route, and for no other. `Route::built_in_name` gives `purlis-<name>` there and the bare name on every other route. The set holds a built-in skill by that name from the start (`skill_set`), so everything that reads the set names it the same way: Use skill… offers and types `/purlis-secrets`, the skill budget lists it, and `skill_list`, `skill_find` and `skill_load` answer with it. Claude Code and Codex are unchanged.
- **Each built-in skill is handed in the set's folder.** `SkillSet::in_the_folder` holds the plane's skills and, on this route, the built-in ones. The host lays the folder out for every opencode chat, with the `skill-studio` flag on or off. Each built-in skill is `skills/purlis-<name>/` there: the `SKILL.md` purlis writes for it, with `name: purlis-<name>` (`handed_text_as`), since the skill format asks that a skill's name be its folder's, and a link to each other file of its folder in the bundle. opencode is handed this folder alone, never the bundle's own `skills/`. Where no folder could be laid out, the chat starts without purlis's skills and the log says so. Handing the bundle's bare names instead would bring back the same-name problem.
- **Turning a skill off goes by the name opencode knows.** `skills_off_for` keeps `purlis-<name>` from ever being denied. A skill of the person's or a clone's named `secrets` is another skill, and is turned off as any is.

## The `purlis-` name of a built-in skill is kept for it on this route

- A plane skill, a repo skill or a harness skill named `purlis-<built-in>` would be a second skill of that name in opencode. Each is left out with one reason, whatever its origin: `Reason::BuiltInsOwnName`, "purlis's own skill goes by this name in this program, and the name is kept for it: rename this skill". purlis can only list a repo's or the person's copy as left out. opencode still reads it, which is why the line asks for the rename.
- Only the `purlis-` names of skills purlis ships are kept. The plane's own names stay as §3 has them: a bare built-in name is still left out on every route, opencode's included ("a built-in skill has this name, and a built-in skill is never replaced"), so a chat's set holds the same plane skills on every harness. A `purlis-` name that no built-in skill has is the plane's to use, and on Claude Code and Codex a plane skill named `purlis-secrets` loads as before.
- A repo or harness skill of a built-in skill's bare name is a skill of its own on opencode, in the set beside purlis's by its own name, and is not left out.
- A persona's `skills:` list still names a built-in skill as `purlis:<name>`, on every harness.

## The index skill is named the same way

- The index skill a router hands past a chat's skill budget (ADR 0102 §2, SK-15) is purlis's too, and on opencode's route it is `purlis-skill-index`: `SkillSet::index` names it by the route (`Index::name`), the set's folder holds it as `skills/purlis-skill-index/`, and its `SKILL.md` says that name. On Claude Code and Codex it is `skill-index`, as before.
- So a clone's or the person's own `skill-index` is a separate skill on opencode, loading beside purlis's under a different name. The studio's line on such a copy, which said opencode could not leave it out by name, is gone. A copy named `purlis-skill-index` is left out with `Reason::BuiltInsOwnName`, as a `purlis-<built-in>` copy is. A plane skill named `skill-index` still never loads, on every route.
- `skills_off_for` never hands opencode `purlis-skill-index` as turned off.

