# Amended (2026-11-05): workspace files may be written from the explorer

From the operator's grill of the workspace-files feature, alongside
[ADR 0113](../0113-a-workspaces-repos-live-in-repos-and-both-layouts-are-read.md). It narrows the
rule the explorer holds (V86 F8: *never create, rename, move or delete*) to repos and their
branches' folders.

- **A repo's files and a branch's folder stay no-write.** They are the agents' work and git's, and
  nothing in this amendment reaches them.
- **The person's own workspace files may be written** from the explorer, with the workspace
  picked: new file, new folder, rename, delete, and import (**Import files…** through the system's
  file picker, and by dropping files from the system's file manager once #4713 is settled).
  - Every write is confined to the workspace's folder, outside `repos/`, `.worktrees/` and any
    clone at the root.
  - The files purlis keeps there (`WORKSPACE_OWN`: its charter, `workspace.json`, memory, todos,
    sessions and the rest) are never written this way. They open read-only, with a link to their
    own view where one exists.
  - Delete sends to the system's Trash after one confirmation, and is refused, never made
    permanent, where there is no Trash.
  - A drop copies. A name already taken gets a ` (2)` suffix, as the system's file manager does.
  - There is no move by dragging inside the tree; rename does that job.
  - A rename or delete of a file a chat is touching asks first.
- **The person's own files open in the light editor**, as a branch's files do, with its stale
  check. No other editing is added: deep work still opens in the person's editor.
