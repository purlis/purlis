# A Regenerate code folder is below a workspace's root or a skill's own folder

**2026-11-11, #6120 (decision A), the operator's ruling of that day: allowlist.** Amends §6 of
ADR 0122 and the amendment of 2026-11-10 that a run reads, of the plane, only its approved code
and its artifact.

A command's code folder is its script's skill's folder (the nearest folder above the script
holding a `SKILL.md`), or else the script's own folder. A run reads that folder, so its approval
is a grant to read all of it. Before this amendment, only a code folder that was the plane's
root or a workspace's root was refused. A `SKILL.md` committed in `<root>/workspaces/`,
`<root>/personas/` or `<root>/skills/`, or a script directly under `workspaces/`, still made
that whole folder the code. Once approved, a run could copy every workspace's memory, sessions
and todos into the page's data. It still needed the viewer's yes, but a list of forbidden
folders refuses only the shapes someone thought of.

- **A code folder is accepted only where this rule names it**
  (`purlis_core::artifact::regenerate::allowed`). Two places qualify:
  - **Strictly below a workspace's root**: `workspaces/<ws>/…`, never `<ws>` itself. The
    folder under `workspaces/` must be a workspace by the rule that lists them
    (`purlis_core::workspaces::is_workspace_folder`): its name has no leading `.`, and it is
    not a real clone. A dot folder or a clone under `workspaces/` is refused.
  - **A skill's own folder, where a level keeps its skills**: `skills/<skill>`,
    `skills.local/<skill>`, `personas/<p>/skills/<skill>` or
    `personas/<p>/skills.local/<skill>`, holding a `SKILL.md`. A workspace's skills are below
    its root already.
- **Everything else is refused, and nothing runs.** That covers the plane's root, a
  workspace's root, `workspaces/`, `personas/`, `skills/`, a persona's own folders outside its
  skill levels, a folder of a skill level that holds no `SKILL.md`, and any other folder of the
  plane. The sentence names the folder, says that Regenerate approves only code below a
  workspace's root or in a skill's folder, and says to move the script into a folder of the
  workspace, such as `scripts/`.
- **The person names a skill's folder by approving it** (the operator's reading of "a skill
  folder the person names", 2026-11-11, relayed on the task). A skill's folder is accepted only at a
  place the plane format already gives a skill (its "workspace and persona skills"), and the
  Regenerate prompt names that folder before the person approves it. That approval is recorded
  per person and per machine in `artifact-regenerate-approvals.json` (`code`), as before. No
  new store is added, and `docs/plane-format.md` is unchanged.
- **What is approved is unchanged**: the code folder's fingerprint and every word of the
  command. A pin whose code folder is now refused stops running until its script moves; its
  recorded approval stays in the record and is never read as standing.
