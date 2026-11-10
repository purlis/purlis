# A named workspace skill on the chat's tab, and in its runs

**Amends [ADR 0099](../0099-a-skill-lives-at-a-level-a-project-skill-reaches-a-workspace-by-grant-and-the-narrowest-copy-wins.md), 2026-11-10**, tickets #6040 (SK-5b) and #6041 (SK-5c). The amendment of the same day ([a persona names another workspace's skill by its workspace](2026-11-10-a-persona-names-another-workspaces-skill-by-its-workspace.md)) left two of its three gaps open: a missing `workspaces/<ws>:<name>` was not on the chat's tab, and a run of the named copy was recorded as the chat's own workspace's. This closes both. Add still does not offer another workspace's skills.

## A missing one is said on the chat's tab

- **The tab's Notice names each, as the persona's list writes it**, the workspace case said apart (decision 41: never a silent drop). A workspace the project does not have: "steward names 1 skill of a workspace the project does not have: workspaces/ghost:x. Fix the workspace's name." A workspace with no skill of that name: "steward names 1 skill the workspace named does not have: workspaces/ops:nope. Make the skill there, or fix the name." Each fix is the one `persona lint` gives, without its command, since the window's fix is the studio.
- **Its fix is the Skill studio on the persona**, as for a missing harness skill: Show in Skills, then Open in Skill studio on the chat's Started without card, which counts these too.
- `skills::NotHere` holds them as `no_workspace` and `not_in_its_workspace`, from `Reason::NoSuchWorkspace` and `Reason::NotInItsWorkspace`.

## A run of the named copy is that copy's

- **The run's row says whose copy it is.** A run at the workspace level, in a chat whose persona names `workspaces/<ws>:<name>` and that works anywhere but `<ws>`, records `copy_workspace` `<ws>` beside its `level`. `workspace` still says where the chat worked, so a skill's totals by workspace are unchanged. `docs/plane-format.md` has the field.
- **It is worked out as the row is written, not carried from the start** (`skills::named_copy_of`). The named copy wins over the chat's own workspace's wherever `<ws>` holds one at the run's level, so the persona's list, the level and the folder say which copy it is. The other way would carry a new field on every route a start takes: the hook's line, a typed start the window holds, `skill_load` and a runner's relayed line. Each of those already reaches the one writer of the row, so the row is the one place that changes. A start does not see SK-9's made-while-a-chat-ran guard here, and neither did it before: the level alone never did.
- **Its content is that copy's.** The `tree` is what git holds of `<ws>`'s folder, so its versions count it.
- **A LOCAL workspace's name is never written.** A run of a LOCAL workspace's named copy keeps `workspace` where it is said and has no `level`, `copy_workspace` or `tree`: it is no copy's, and never the chat's own workspace's copy by mistake.
- **Where the copy's runs are read, they are told apart by it.** A skill's history counts a workspace copy's runs wherever they ran (`RunCopy::workspace_of_copy`). So does the suggestion to deprecate a skill no chat ran. `skill_runs` shows a chat the runs of its own copy only: the named copy's, or the chat's own workspace's, never both.

## The studio acts on the copy in its own workspace

A studio row of the named copy carries its workspace (`StudioRow::copy_workspace`, the window's `copy_workspace`). Its label (`workspace ops`), its source label, and its changes (edit, share, history, and the rest that take a level) are of `<ws>`'s copy, whatever the studio shows. Before, the studio as the persona gave the row no level, so none of its changes were offered. Use skill… labels the copy by its source the same way.
