# Amended 2026-10-20: Create a theme is purlis's fourth action and the plane's first (spec #2313, #2333)

The theme spec (#2313, "The skill") names a fourth action of purlis's own:

- `charter/create-theme`, "Create a theme", on workspaces and the plane: *Use purlis's theme
  skill to create a theme for the {subject.kind} {subject.name}.* It runs in the workspace's
  directory, or at the plane root on the plane, because a chat's theme draft is drawn where the
  chat works and a new theme is kept at that level. It is the first of purlis's own on the plane,
  and it comes after the other three in every list.

**Settings › Theme opens the same chat.** The gallery's **New theme** opens Create a theme on the
level the gallery is at (a workspace, or the project at the plane root), with the same prompt.
**Edit with a chat** opens it with one more thing named: the chosen card's theme, by the id
`purlis theme list` gives it (`purlis_core::themechat`). The card is one the gallery shows at
that level; an id with anything but the letters, digits and marks an id is spelled with opens
nothing. Both are the window's alone (`theme_chat`), typed and never sent, by every rule of this
record.
