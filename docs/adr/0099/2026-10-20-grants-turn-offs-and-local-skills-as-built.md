# Grants, a workspace's turn-offs and local skills, as #2767 built them

**Amends [ADR 0099](../0099-a-skill-lives-at-a-level-a-project-skill-reaches-a-workspace-by-grant-and-the-narrowest-copy-wins.md), 2026-10-20**, ticket #2767 (SK-4). ADR 0099 left the shape of a workspace's turn-off and the git rule for `skills.local/` to this ticket. This records how they were settled.

## A workspace turns a skill off in its own settings, and only off

- The list is `settings.skills.off` in the workspace's `workspace.json`, a list of skill names. It is one more table of the Workspace settings layer (ADR 0048). A save is refused for any other key under `skills`, and for an `off` that is not a list of names. The reader then turns nothing off.
- **There is no `on` list.** A name in `off` whose skill is not granted to the workspace stays "not granted to this workspace". `purlis skill grant <name> -w <ws> --on` only takes a name back out of `off`, and is refused, with the grant that would reach the workspace, for a skill the project did not grant it.
- The list reaches project skills alone. A workspace skill is the workspace's own and has no switch. A built-in skill cannot be turned off.
- `charter.toml` and `charter.local.toml` hold no `[skills]`. Which workspaces a project skill reaches is the skill's grant, beside its folder. So the project has one place to say where a skill reaches, and each workspace has one place to say what it turned off.

## A local skill is read only while git ignores its `SKILL.md`

- A level's `skills.local/<name>/` is read only where `git check-ignore` says its `SKILL.md` is ignored, or where the plane is no git repository. This is the same rule as `charter.local.toml`'s. A tracked file counts as not ignored.
- A folder git would commit, or one git could not answer for, is left out with git's words and the fix, in the skill set's one line. It is never read quietly.
- **Two guards keep a local skill out of git.** `init` writes `/skills.local/` and `/workspaces/*/skills.local/` into the plane's `.gitignore`, and `reinit` adds them to an older plane. A save's stage and the standing's status also exclude every level's `skills.local/`, whatever the `.gitignore` says. A save stages the whole tree, so one lost line would otherwise commit a person's local skills to the team (#2874). The left-out line of a local skill git would commit says that `purlis reinit` keeps it on this machine.
- **Share** moves the folder, with its `<name>.toml`, from `skills.local/` into `skills/` at the same level, and refuses when that folder name is taken there. A local project skill reaches workspaces by its own grant, as a shared one does.

## The narrowest copy, in order

The levels are read narrowest first: the workspace's local skills, its shared skills, the project's local skills, then its shared skills. The first copy of a name is the one handed over. Every other copy is listed, widest first, with "hidden by the <level> skill of this name". Two folders of one level that name one skill keep SK-2's "hidden by another <level> skill of this name". A narrower copy that is itself left out (a local one git would commit, for example) hides nothing.
