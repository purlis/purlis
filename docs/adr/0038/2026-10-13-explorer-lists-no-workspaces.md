# Amendment, 2026-10-13: Explorer lists no workspaces

The operator's words of 2026-10-13, on the Explorer view in the left sidebar: _"workspaces
selector is very redundant thing, as workspaces we have in top bar separate tabs, we need to just
show current workspace files, and this item in sidebar can be like a files browser only - without
workspaces selector - only selected workspace"_. This takes back the first of the three sections
of the amendment of 2026-10-10 above.

- **The Workspaces section is gone.** The strip is the one place a workspace is selected (ADR
  0036). What a row of the section carried is on the workspace's tab, where it already was: the
  press that focuses it (the same catalogue row), its menu, and its live mark. Nothing was
  reachable only from the section.
- **Explorer is the files of the workspace the strip is on, and follows the strip.** In the
  window, the Explorer view is Files alone; Repos and branches stays the Panel's Repos view. The
  Files heading names whose files they are.
- **On the plane root's tab** the heading says _Plane root_ and the section says it lists a
  workspace's files. The operator's ruling of the same day is that this tab shows the whole
  plane's files. That needs the core to read folders that are no workspace's repo or branch,
  which it refuses today by design (`files.rs`, where a repo's own folder is found), so it is a
  decision about what the window may read and is not made in this change.
- **A layout file that folded the section** (`explorer.closed` holding `workspaces`) is read past
  without a word, and the next write leaves the name out.
