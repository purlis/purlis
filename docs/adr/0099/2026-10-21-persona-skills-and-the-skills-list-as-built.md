# Persona skills and the `skills:` list, as #2768 built them

**Amends [ADR 0099](../0099-a-skill-lives-at-a-level-a-project-skill-reaches-a-workspace-by-grant-and-the-narrowest-copy-wins.md), 2026-10-21**, ticket #2768 (SK-5). ADR 0099 §1 and §2 said a persona's skills, and the skills it names, reach its chats whatever the workspace was granted. This records how a name is read, what a workspace can still take away, and where a missing skill is said.

## A persona's own skills are read like a workspace's

- `personas/<p>/skills/<name>/` and `personas/<p>/skills.local/<name>/` are the persona level. They are read for every chat as `<p>`, at the project's root and in every workspace, before the workspace's and the project's levels, so a persona copy wins a same-name tie and every other copy says "hidden by the persona skill of this name". A built-in skill's name is still never taken.
- `skills.local/` is read under #2767's git rule. A flat `personas/<p>.md` persona has no folder and so no skills of its own.

## The `skills:` line names skills, and a chat as the persona loads them

Each comma-separated entry of the persona's own `skills:` line is one `purlis_skills::Reference`:

| Written | Names | A chat as the persona |
|---|---|---|
| `purlis:<name>`, `charter:<name>` | a built-in skill | has it already; one there is none of is left out, "no built-in skill has this name" |
| `<plugin>:<name>` | a skill of a harness plugin | the harness loads it; where the chat's harness does not have it on this machine, it is left out, "not installed on this machine" |
| `<name>` | the plane skill of that name, else the harness's own skill of that name | gets a project skill of that name **whatever its grant**; a persona or workspace copy wins as always; where no plane level has it and the harness does not either, it is left out as above |

- **`charter:` is read as `purlis:`** until the migration (#2784) rewrites it, so no persona written before the rename loses a skill. The page writes `purlis:`.
- **A workspace's turn-off still holds.** A project skill a persona names reaches a workspace it was not granted to, but one the workspace turned off stays "turned off in this workspace". Grilled and decided: the grant is the project's statement of where a skill may go, which a persona's name overrides as decision 17 says; the turn-off is that workspace's own explicit choice, and decision 3 says a workspace "can turn a granted skill off". Overriding the person's last word with a persona's file would make the turn-off mean nothing for exactly the chats that carry the skill.
- **A workspace skill of another workspace cannot be named.** `<name>` resolves in the chat's own workspace. Naming one by its workspace waits on a real need (#2933).
- **Harness skills are looked for, never handed over.** The harness's declaration lists what it has on this machine (`harness_plugin::Adapter::skills`): Claude Code's own skills, its synced ones and each installed plugin's `skills/`; Codex's `$CODEX_HOME/skills`, `~/.agents/skills` and each recorded plugin as a whole; opencode's `~/.claude/skills`, `~/.agents/skills` and its config folders' `skill/` and `skills/`. A plugin whose skills cannot be listed counts as having every skill named under it, so a chat is never told it lacks one it has.

## A missing skill is said on the chat's tab

At a chat's start the host asks for the set with the harness skills listed (`hoststart::skills_not_here`), and every skill the persona names that is "not installed on this machine" or "no built-in skill has this name" goes into one line on the chat's tab, naming each. The chat starts without them. The Skill studio and the persona page are read without a harness, so they never say a harness skill is missing: whether it is depends on the chat's harness.

## `persona stats` reads the loaded list

`persona stats`' drift compares the skills a persona used with what it loads: the names of its `skills:` line, as before, and its own skills. An own skill used is no longer "undeclared".
