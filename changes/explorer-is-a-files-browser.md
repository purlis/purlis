### Changed

- **Explorer no longer lists the workspaces.** The left sidebar's Explorer view had a Workspaces
  section above Files, the same list as the workspace tabs at the top of the window. It is gone:
  Explorer is the files of the workspace selected on the strip, and follows the strip. The Files
  heading still names whose files they are. A workspace's live mark and its menu (rename, Safe
  remove and the rest) are on its tab, as they always were. On the plane root's tab, Files says
  it lists a workspace's files and names the plane root in its heading. A layout file that had
  the Workspaces section folded is read as before, and nothing is said about it.

### Fixed

- **A folder's chevron is in the middle of its row.** In Explorer's Files, in a file tab's tree
  and on a repo's row in Repos, the `>` beside a folder stood above the row's middle, closed or
  open, at every depth. It is now centred on the row's line, in every tree that draws one.
