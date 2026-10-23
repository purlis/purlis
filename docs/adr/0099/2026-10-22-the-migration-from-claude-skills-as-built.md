# The move from `.claude/skills` into the project's skills, as #2784 built it

**Amends [ADR 0099](../0099-a-skill-lives-at-a-level-a-project-skill-reaches-a-workspace-by-grant-and-the-narrowest-copy-wins.md), 2026-10-22**, ticket #2784 (SK-21), spec #2762 decision 31. ADR 0099's Consequences said the copy of the plane's `.claude/skills` into clones retires with a migration that moves those skills to project skills granted `"all"`. Its amendment of 2026-10-21 (persona skills) said `charter:` is read as `purlis:` until that migration rewrites it. This file records how the migration was built, and the dispatcher's ruling of 2026-10-22 on where the Skill studio's flag stops.

## A moved project's skills reach every chat, flag on or off

**The dispatcher's ruling on #2784's review:** the `skill-studio` flag hides only the studio. Once a project's manifest requires `project-skills`, the project's skills reach every chat on any build that knows the feature, with the flag on or off. Otherwise a teammate with the flag off would lose every skill when the copy into clones retires.

- `skills::reach_chats(plane, studio)` is the one question every chat's start, restart offer, missing-skill Notice and skill tool asks: the flag is on, **or** the project requires `project-skills`. Only the studio's own surfaces still go by the flag alone: the view, `Use skill…`, `purlis skill`, Settings' summary row, and the move itself.
- A build without the feature opens a moved project read-only (`compat::KNOWN`). No build can therefore start a chat in it without its skills.

## One fix, one commit, undone by `git revert`

- The migration is the doctor fix **`project-skills`**: `purlis doctor --fix project-skills`, run only by its name, because it is a commit every teammate pulls. It shares `fix/commit.rs` with `rename-plane`: the claim, the refusals and the commit of exactly its own paths through purlis's git runner, which runs no hooks.
- **Refused before anything is touched:** from inside a chat; anywhere but a project's root (no manifest, or a project above it, such as a workspace's clone); while the Skill studio's preview is off on this machine (the move is part of the preview); on a project this purlis may not write; outside a git repository whose top is the project; with tracked changes not committed; on a detached HEAD; and in the middle of a merge, a rebase, a cherry-pick or a revert.
- **What it moves:** each folder of `.claude/skills/<name>/`, with `git mv`, to `skills/<name>/`, and `skills/<name>.toml` made new through the folder's handle with `grant = "all"`. Every clone got a copy of those skills, so `"root"` would take them from every workspace's chats.
- **What it rewrites:** each `charter:<skill>` reference in a persona's definition and in the `.md` files of every skill folder of the plane. A reference is the prefix followed by a skill's name, not after a word character, a dot, a slash, an `@` or a `:`, and never on a line of a fenced code block. So `ghcr.io/acme/charter:v1` and quoted code are left as written. `rename-plane` reads the same rule.
- **What it resolves:** every persona's `skills:` list, read as a chat as that persona reads it. Each entry that names no skill a chat can get is reported with its one line. Such an entry was unresolved before the move too, so it is reported and never refused.
- **What it marks:** the manifest's `requires` gains `project-skills`.
- **All or nothing.** When any step fails before the commit is made (the commit included, for example when a signer refuses), the index goes back to `HEAD` whole and each moved folder and rewritten file is checked out from `HEAD`. What the fix made (the folders' new copies, the grant files, `skills/` itself where it made it) is removed through its folder's handle by the names it gave them. The project had no tracked change when the fix began, so this is exactly where it was. The commit is undone whole with `git revert`, which the report names.

## Every chat keeps its skills, or nothing is written

- **Before writing**, it refuses what a move cannot carry honestly:
  - a file git does not track under `.claude/skills`;
  - a committed link or submodule;
  - a file beside the skill folders;
  - a `skills/<name>` or `skills/<name>.toml` that is already there;
  - a folder that is no whole skill (no `SKILL.md`, or one without `name` or `description`), which every chat would leave out;
  - a skill named as a built-in skill in any case;
  - a name a project skill already has;
  - two folders that name one skill.
- **After moving and before committing**, it works out the skill set of every chat it can name, and puts everything back unless each moved skill is in every one. The chats are those at the project's root and in each workspace, as no persona and as each persona, on each of the four routes (plugin, config, briefing, MCP). They also include each chat in each clone, with that clone's own committed skills on each program's route. A moved skill may be left out only where a workspace turned it off or a narrower copy of its name stands in for it.
- **A clone's own committed skill of the same name is said, never hidden.** Its chats get the repo's copy, as they did before (#2769's tie rule).
- Claude Code and opencode keep exactly the skills they had, at the root and in a clone. Codex gains them, since it never read `.claude/skills`. `project_skills_move_without_a_chat_losing_one.rs` shows this on a real plane with a real clone.

## The copy into clones retires with the move, not before

- The guest layer copies the plane's `.claude/skills` into a checkout only **while the project does not require `project-skills`** (`guest::RETIRED_MIRROR`). A project that has not moved keeps its copy, so no clone loses its skills before the move.
- Once the project has moved, every `.claude/skills` copy purlis made counts as one the project stopped having. Each is withdrawn by #1583's rules: recorded, unedited, untracked, and a text the project offered there.
- **The fix refreshes this machine's clones at once.** A teammate's clones refresh the next time a chat starts in each one. A copy somebody edited is kept, and the fix names it.

## What was rejected

- **Retiring the copy for every plane at once.** A plane that has not moved would lose its skills in every clone without a word.
- **Keeping project skills behind the flag after the move.** A teammate with the flag off would start every chat without them (the ruling above).
- **A `purlis skill migrate` command.** The doctor's fix registry is where a one-off rewrite of committed files lives (`rename-plane`, `persona-agents`). The Skill studio's first open (SK-22, decision 42) is the window's way to it.
- **Moving a link, an untracked file or a folder that is no whole skill as it is.** The first changes meaning in its new folder, the second is never committed, and the third no chat would get. All three are refused, so the person decides what they become.
- **Rewriting every `charter:` in text.** An image tag or quoted code is not a reference.
