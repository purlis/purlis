# Harness and repo skills, as #2769 built them

**Amends [ADR 0099](../0099-a-skill-lives-at-a-level-a-project-skill-reaches-a-workspace-by-grant-and-the-narrowest-copy-wins.md), 2026-10-21**, ticket #2769 (SK-6), spec #2762 decisions 5, 25, 35, 36 and 51. §1 said harness and repo skills are listed and can be turned off where the harness allows; §3 said their ties are the harness's. This records how.

## Two origins, read only, never handed

- **A harness skill** is one a harness finds in the person's own folders for it, or in one of its installed plugins. **A repo skill** is one a clone in the workspace holds where a harness reads a repo: `.claude/skills` for Claude Code; `.agents/skills` and `.codex/skills` for Codex; `.claude/skills`, `.agents/skills` and `.opencode/skill(s)` for opencode. Each harness's adapter names its folders (`harness_plugin::Adapter::skills_at`, which #2768 added for a persona's named skills, and `repo_skill_dirs`), and `harness_plugin::harness_skills` and `harness_plugin::repo_skills` read them, each into an `OutsideSkill` with its `Owner`.
- They are `Origin::Harness` (the origin #2768 gave a persona's named harness skill) and `Origin::Repo` in `purlis-skills`, by the `Route::Own`: the harness reads them where they lie, and purlis hands none of them. `SkillSet::from_the_plane` leaves them out, so no route ever links or briefs one. `SkillSet::with_outside` lists them beside the plane's and settles each clash as the chat's route allows.
- Each is listed by the name its harness knows it by: its `name`, or `<plugin>:<name>` for a plugin's (Claude Code and Codex both namespace a plugin's skills that way). Each says whose it is: "you", the plugin's name, or the repo's name.
- **A clone is the chat's to write, so nothing in it is read through a link.** Each folder on the way to a repo skill and its `SKILL.md` is opened through the handle of the folder above it, with no link followed at its last step. A skill folder that is a link is listed by its name, saying purlis does not follow it, since the harness does. The person's own folders are theirs, and a link there is read where it leads.
- What a repo holds is listed as it is on disk, committed or not: the harness loads it either way.

## A clash: the narrowest copy wins wherever purlis chooses

A skill from outside the plane whose name is a skill purlis hands the chat is a **clash**. Decision 51 holds: the narrowest copy wins wherever purlis chooses, and a repo skill is narrower than the plane's.

- **On opencode and Codex** purlis builds what the harness is handed (the folder opencode is told, the list a Codex chat is briefed), so a chat in a clone is handed one copy of the name: the repo's. The plane's copy is left out of that chat's set, "hidden by the repo skill of this name", read by the host from the clone the chat starts in (`skills::repo_skills_of_chat`) and handed to the adapter, which only turns the set into words. With one copy of a name handed over, opencode's `permission.skill`, which goes by the name alone, is never ambiguous between the plane's copy and the repo's.
- **On Claude Code** the plane's skills come as their own namespaced `--plugin-dir` plugin, so both load, and the studio marks the plane's copy ("a chat there gets both").
- **A harness skill** (the person's own, or a plugin's) is wider than the plane's, and purlis cannot keep the harness from reading it, so both load on every harness and the plane's copy is marked. The person can turn the other copy off where the harness allows it.
- A plugin's namespaced skill clashes with nothing.

- **A built-in skill never loses**, to a repo copy or a harness copy alike, on any route: purlis's is the one handed, and the other is listed "hidden by purlis's built-in skill of this name". A clone cannot take purlis's own `secrets` away from a chat.

"Rename the project's copy", on the mark, opens the studio's own Rename, which is #2932's (`skill_rename`): one rename, with every persona and reference that names the skill.

## The switches

- One table, `[harness_skills.<harness>]`, on the same layers and precedence as `[harness_plugins]` (ADR 0050 as amended 2026-10-21, which records each harness's switch and its measurement).
- **The Skill studio writes it**, beside a harness's plugins: at the project's root in `charter.local.toml`, since a harness's installs are the person's own and a choice about them should not travel to the team; in a workspace in its `workspace.json`, which is the team's for that workspace (decision 36). It writes the least that makes the answer what the person chose: turning one back on takes this layer's key out, and writes `true` only where another layer would still say off.
- **Settings' Plugins group is one summary row** while the studio's flag is on, counting each harness's plugins on, off and left to it, with Open in Skill studio (decision 35). With the flag off it keeps its controls.
- **A built-in skill** is listed as always on: it ships with purlis and cannot be turned off, and its name is never taken (§3). Its name is refused in `[harness_skills.<harness>]` on every harness, and never handed to one as turned off: Claude Code's and opencode's switches go by the name alone, so one named so would reach purlis's own.
