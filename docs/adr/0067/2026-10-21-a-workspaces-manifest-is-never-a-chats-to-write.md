# Amended 2026-10-21: a workspace's manifest is never a chat's to write

**Built by #2046**, on §2 (brokered writes) and §5 (later code), following the proposed-change
rulings of 2026-10-14 (PC-1 to PC-14): a chat's change to purlis's own configuration is made by
the person. *Decided by the steward on 2026-10-21 under those rulings.*

1. **Held at both levels.** A chat's sandbox never writes `workspaces/<workspace>/workspace.json`,
   the file the app reads a workspace's repos and settings from, under any spelling of its name:
   - in the full sandbox, each of the project's workspaces' by its path
     (`purlis_core::sandbox::workspace_manifests_held`), beside the project's manifests;
   - at the floor, every project's by pattern, `…/workspaces/<any>/workspace.json` in any case
     (`seatbelt::workspace_manifests_regex`, and the same glob in Claude Code's settings).
2. **What a chat's asks need, the app writes.** A chat's `purlis clone` is made by the app, which
   records the repo in the workspace's manifest (the clone ask of §2). A new workspace is made by
   the app too (`WorkspaceCreate`).
3. **The rest is the person's.** `purlis workspace rename`, `fork` and `snapshot`, and
   `purlis skill grant --on/--off`, each of which rewrites a workspace's manifest, are refused in
   a sandboxed chat in words, and nothing is changed; the person runs them in a terminal, or
   changes the setting in the window. Where a sandboxed chat's command adds a workspace's
   structure, its manifest is left for the app to scaffold.
