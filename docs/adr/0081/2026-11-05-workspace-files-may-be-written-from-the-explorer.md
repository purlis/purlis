# Amended (2026-11-05): workspace files may be written from the explorer

From the operator's grill of the workspace-files feature, alongside
[ADR 0113](../0113-a-workspaces-repos-live-in-repos-and-both-layouts-are-read.md). It narrows the
rule the explorer's tree holds (V86 F8: it creates, renames, moves and deletes nothing) to repos,
branches' folders and the plane root.

- **The tree still creates, renames, moves and deletes nothing in a repo, a branch's folder or the
  plane root** (the 2026-10-13 amendment). They are the agents' work and git's, and nothing in this
  amendment reaches them. The light editor's small edits there are unchanged.
- **The person's own workspace files may be written** from the explorer, with the workspace
  picked: new file, new folder, rename, delete, and import (**Import files…** through the system's
  file picker, and by dropping files from the system's file manager once #4713 is settled).
  - Every write is confined to the workspace's folder, outside `repos/`, `.worktrees/` and any
    clone at the root.
  - The files purlis keeps there are never written this way: the reserved workspace names
    (`WORKSPACE_OWN`: its charter, `workspace.json`, memory, todos, sessions and the rest),
    `skills.local/`, the folders an extension keeps in the workspace, and purlis's generated
    `.claude/` and `.purlis-*` files. They open read-only, with a link to their own view where one
    exists.
  - Delete sends to the system's Trash after one confirmation, and is refused, never made
    permanent, where there is no Trash.
  - A drop copies. A name already taken gets a ` (2)` suffix, as the system's file manager does.
  - There is no move by dragging inside the tree; rename does that job.
  - A rename or delete of a file a chat is touching asks first.
- **The person's own files open in the light editor**, as a branch's files do, with its stale
  check. No other editing is added: deep work still opens in the person's editor.
