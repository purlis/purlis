# Amendment, 2026-09-26: the plane root is always the strip's first tab (SI-1)

The workspace strip draws the pinned workspaces and the one you are in, and — until this
amendment — a tab for "Outside every workspace" only while some chat was working in no
workspace. The operator ruled on 2026-09-26 that the plane root is a place of its own: the chat
that looks after the plane, its personas and its workspaces works there. So:

- **The plane root is always drawn, and always first**, before the pins. It is never behind
  show-more: its width is taken off the strip's room before the workspaces share what is left.
- **It is an icon, not a name** (`FolderRoot`), with the tooltip *"Plane — chats here start at
  the plane root"*. Its accessible name, and its name in the palette and menus, is **Plane
  root** (CONTEXT.md).
- **It cannot be dragged, pinned, renamed or deleted**: it is not a directory under
  `workspaces/`, so nothing on disk or in the machine store names it. Its menu is its own —
  focus it, a new chat there, a new shell there, and a new workspace.
- **Chats and shells started from it start in the plane's own directory**, and the chat is told
  it is in no workspace (`$PURLIS_PLANE_ROOT_SESSION`, `docs/plane-format.md`). The panels that
  are a workspace's say that the plane root is not one, rather than drawing another's.

A launch with nothing in front still lands on the first workspace, as before; with no workspace
at all it lands on the plane root.
