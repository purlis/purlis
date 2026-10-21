# Persona skills and the `skills:` list, as #2768 built them

**Amends [ADR 0099](../0099-a-skill-lives-at-a-level-a-project-skill-reaches-a-workspace-by-grant-and-the-narrowest-copy-wins.md), 2026-10-21**, ticket #2768 (SK-5). ADR 0099 §1 and §2 said a persona's skills, and the skills it names, reach its chats whatever the workspace was granted. This records how a name is read, that a workspace cannot take a named skill away, where the skills are chosen, and where a missing skill is said.

## A persona's own skills are read like a workspace's

- `personas/<p>/skills/<name>/` and `personas/<p>/skills.local/<name>/` are the persona level. They are read for every chat as `<p>`, at the project's root and in every workspace, before the workspace's and the project's levels, so a persona copy wins a same-name tie and every other copy says "hidden by the persona skill of this name". A built-in skill's name is still never taken.
- `skills.local/` is read under #2767's git rule. A flat `personas/<p>.md` persona has no folder and so no skills of its own.

## The `skills:` line names skills, and a chat as the persona loads them

Each comma-separated entry of the persona's own `skills:` line is one `purlis_skills::Reference`:

| Written | Names | A chat as the persona |
|---|---|---|
| `purlis:<name>`, `charter:<name>` | a built-in skill | has it already; one there is none of is left out, "purlis has no built-in skill of this name" |
| `<plugin>:<name>` | a skill of a harness plugin | the harness loads it; where the chat's harness does not have it on this machine, it is left out, "not installed on this machine"; where purlis cannot list the plugin's skills, it is kept and marked unchecked |
| `<name>` | the plane skill of that name, else the harness's own skill of that name | gets a project skill of that name **whatever its grant and whatever the workspace turned off**; a persona or workspace copy wins as always; where no plane level has it and the harness does not either, it is left out as above |

- **`charter:` is read as `purlis:`** until the migration (#2784) rewrites it, so no persona written before the rename loses a skill. The page writes `purlis:`.
- **A workspace's turn-off does not reach a skill the persona names** (the dispatcher's ruling R1 of 2026-10-21, over this ticket's first draft, which kept the turn-off). The spec says a persona reference to a project skill works in every workspace, and story 17 that dispatching a persona never strips its skills. The turn-off still holds for every chat in that workspace that is not that persona's.
- **A workspace skill of another workspace cannot be named.** `<name>` resolves in the chat's own workspace. Naming one by its workspace waits on a real need (#2933).
- **Harness skills are looked for, never handed over.** The harness's declaration says where it finds skills (`harness_plugin::Adapter::skills_at`), reading only its own record of what is installed, as `Adapter::installed` already does; the host lists the skills there (`skills::on_machine`). Claude Code: its own `skills/`, its synced `skills/synced/` and each installed plugin's folder from `installed_plugins.json`. Codex: `$CODEX_HOME/skills`, `~/.agents/skills` and each recorded plugin in `plugins/cache/<marketplace>/<plugin>/<version>/`. opencode: `~/.claude/skills`, `~/.agents/skills` and its config folders' `skill/` and `skills/`. A plugin's skills are where its manifest's `skills` key says, else in `skills/`.
- **A plugin whose skills cannot be listed is never silent.** Its folder is not there, or the record names none: a skill named under it is neither said to be there nor missing. The studio's row says in one line that purlis can't check it, and no Notice is raised.

## Every skill choice is the Skill studio's

The persona page shows one row, "Brings N skills into every workspace", which opens the Skill studio filtered to a chat as the persona (decision 35, the dispatcher's ruling R2 of 2026-10-21, over the ticket's older wording of a Skills section on the page). There the persona's own skills, the skills it names and the installed skills it names are listed with their level; Add names one more and Remove takes a name out, editing the persona's own `skills:` line. An installed skill the persona names is checked against every harness the project's profiles run (Claude Code where none is offered): there where any has it, unchecked where none has it and one cannot list its plugin's skills, and left out only where none has it.

## A missing skill is a Notice on the chat's tab

At a chat's start the host asks for the set with the chat's harness skills listed (`hoststart::skills_not_here`). Every installed skill the persona names that is "not installed on this machine", and every built-in skill purlis has none of, goes into one Notice on the chat's tab, each named, the built-in typo said apart (decision 41, the dispatcher's ruling R3). Its fix opens the Skill studio on the persona; Dismiss puts it away. The chat starts without them. A chat on a runner, Docker or k8s is not checked yet (#2977).

## `persona stats` reads the loaded list

`persona stats`' drift compares the skills a persona used with what it loads: the names of its `skills:` line, as before, and its own skills. An own skill used is no longer "undeclared".
