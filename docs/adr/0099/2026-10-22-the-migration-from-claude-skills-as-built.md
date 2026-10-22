# The move from `.claude/skills` into the project's skills, as #2784 built it

**Amends [ADR 0099](../0099-a-skill-lives-at-a-level-a-project-skill-reaches-a-workspace-by-grant-and-the-narrowest-copy-wins.md), 2026-10-22**, ticket #2784 (SK-21), spec #2762 decision 31. ADR 0099's Consequences said the copy of the plane's `.claude/skills` into clones retires with a migration that moves those skills to project skills granted `"all"`, and its amendment of 2026-10-21 (persona skills) said `charter:` is read as `purlis:` until that migration rewrites it. This records how the migration was built.

## One fix, one commit, undone by `git revert`

- The migration is the doctor fix **`project-skills`**: `purlis doctor --fix project-skills`, run only by its name, because it is a commit every teammate pulls. It sits beside `rename-plane` and shares its rules and its helpers: refused from inside a chat, on a project this purlis may not write, outside a git repository whose top is the project, and with tracked changes not committed. Everything is changed in the working tree and committed with exactly its own paths through purlis's git runner. A failure before the commit puts every folder and file back, and the commit is undone whole with `git revert`, which the report names.
- **It also refuses while the Skill studio's preview is off on this machine.** With the flag off a chat gets none of the project's skills, so a move would lose them.
- **What it moves:** each folder of `.claude/skills/<name>/`, with `git mv`, to `skills/<name>/`, and `skills/<name>.toml` with `grant = "all"`. A skill was copied into every clone, so `"root"` would take it from every workspace's chats.
- **What it rewrites:** each `charter:<skill>` reference in a persona's definition and in the `.md` files of every skill folder of the plane, by `rename-plane`'s rule (the prefix followed by a skill's name, and not after a word character). Scripts and other files are left alone, since a rewrite there could change what they run. Prose such as "charter: no" is never touched.
- **What it resolves:** every persona's `skills:` list, read as a chat as that persona at the project's root reads it (`skills::in_the_studio`). Each entry that names no skill a chat can get is reported with its one line ("purlis has no built-in skill of this name", "not installed on this machine"). Such an entry was unresolved before the move as well, so it is reported and never refused.
- **What it marks:** the manifest's `requires` gains `project-skills` (`skills::PROJECT_SKILLS_FEATURE`, added to `compat::KNOWN`). A build without project skills opens the project read-only instead of starting every chat without its skills.

## Every chat keeps its skills, or nothing is written

- **Before writing**, it refuses what a move cannot carry honestly: a file git does not track under `.claude/skills` (it would move uncommitted, or stay behind), a committed link or submodule (a link's target changes meaning in another folder), a file beside the skill folders, a `skills/<name>` or `skills/<name>.toml` already there, and a skill that would be left out of every chat once moved: one named as a built-in skill, one whose name a project skill already has, and two folders that name one skill.
- **After moving and before committing**, it works out the skill set of a chat at the project's root and in each workspace (`skills::of_a_chat`). Unless each moved skill is there, it puts everything back. A moved skill may be missing only where the workspace turned it off or a narrower copy of its name stands in for it.
- **Claude Code and opencode keep exactly the skills they had**, at the root and in a clone. Before, they read `.claude/skills` where it lay and in purlis's copy. After, each is handed the skills by its route. **Codex gains them**, since it never read `.claude/skills`. The integration test `project_skills_move_without_a_chat_losing_one.rs` shows this on a real plane with a real clone, on all three programs.

## The copy into clones retires with the move, not before

- The guest layer copies the plane's `.claude/skills` into a checkout only **while the project does not require `project-skills`** (`guest::RETIRED_MIRROR`, `mirrors`). A plane that has not moved keeps its copy, so no clone loses its skills before the move. Once the move is committed, every `.claude/skills` copy purlis made counts as one the project stopped having, whatever the plane's own `.claude/skills` holds later. Each is withdrawn at the next wire by #1583's rules: recorded, unedited, untracked, and a text the project offered there.
- `.claude/agents` is still copied. `WALKUP_DIRS` keeps both roots, so a record that names an old copy is still recognised and withdrawn.

## What was rejected

- **Retiring the copy for every plane at once.** A plane that has not moved would lose its skills in every clone without a word, which decision 31 rules out.
- **Turning the copy off with the Skill studio's flag.** That reads one machine's switch, while the copy is about the project's committed files. Teammates with the flag in different states would get different clones.
- **A `purlis skill migrate` command.** The doctor's fix registry is where a one-off rewrite of committed files lives (`rename-plane`, `persona-agents`). The Skill studio's first open (SK-22, decision 42) is the window's way to it.
- **Moving a link or an untracked file as it is.** The first changes meaning in its new folder, and the second is never committed. Both are refused so the person decides what they become.
