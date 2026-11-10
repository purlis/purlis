# A named workspace skill on the chat's tab, and in its runs

**Amends [ADR 0099](../0099-a-skill-lives-at-a-level-a-project-skill-reaches-a-workspace-by-grant-and-the-narrowest-copy-wins.md), 2026-11-10**, tickets #6040 (SK-5b) and #6041 (SK-5c). The amendment of the same day ([a persona names another workspace's skill by its workspace](2026-11-10-a-persona-names-another-workspaces-skill-by-its-workspace.md)) left two of its three gaps open: a missing `workspaces/<ws>:<name>` was not on the chat's tab, and a run of the named copy was recorded as the chat's own workspace's. This closes both. Add still does not offer another workspace's skills.

## A missing one is said on the chat's tab

- **The tab's Notice names each, as the persona's list writes it**, the workspace case said apart (decision 41: never a silent drop). A workspace the project does not have: "steward names 1 skill of a workspace the project does not have: workspaces/ghost:x. Fix the workspace's name." A workspace with no skill of that name: "steward names 1 skill the workspace named does not have: workspaces/ops:nope. Make the skill there, or fix the name." Each fix is the one `persona lint` gives, without its command, since the window's fix is the studio.
- **Its fix is the Skill studio on the persona**, as for a missing harness skill: Show in Skills, then Open in Skill studio on the chat's Started without card, which counts these too.
- `skills::NotHere` holds them as `no_workspace` and `not_in_its_workspace`, from `Reason::NoSuchWorkspace` and `Reason::NotInItsWorkspace`.
