# Amended (2026-11-05): with the workspace picked, Explorer shows the workspace's folder

From the operator's grill of the workspace-files feature, alongside
[ADR 0113](../0113-a-workspaces-repos-live-in-repos-and-both-layouts-are-read.md) and ADR 0081's
amendment of the same day. It extends #3343's *Explorer opens straight to the files* (repos as
folders) and adds no fold and no icon.

- **With the workspace itself picked, Explorer's tree is the workspace's folder as it is on
  disk**: the workspace files, and its repos as folder rows, as #3343 draws them. Where `repos/`
  exists the repo rows sit inside it; a clone still at the root is a repo row where it stands.
  Picking a repo or a branch narrows the tree to it, as before. The Panel's Repos view is
  unchanged.
- **The files purlis keeps there carry a *managed* mark** and open read-only, with a link to their
  own view where one exists. `.worktrees/` and other hidden files are shown only with **Show
  hidden**.
- **The workspace files have the tree's filter, its watch, Add to chat, Copy path, Reveal and Open
  in editor**, and the writes ADR 0081's amendment allows, with **Import files…** through the
  system's file picker. Dropping files in from the system's file manager follows once #4713 is
  settled.
- **In a LIVE workspace each file says where it is kept**: *This machine only*, or *Saved with the
  project* when the LIVE allowlist holds it. **Save with the project** adds one file or folder to
  the allowlist; nothing is ever added without that press, since a script may hold a token.
- **A clone still at the root raises no notice.** The old layout is supported for ever, so it is
  not out of date: `doctor`'s row is the one place it shows.
- **The plane root's tab is unchanged**: Files is the whole project, and a repo, wherever it
  lives, is one row that leads to its workspace.
