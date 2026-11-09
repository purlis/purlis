# Amendment, 2026-11-09: a workspace's chip wears no LIVE mark (#5920)

[The title bar's amendment](2026-10-24-the-workspaces-move-to-the-title-bar.md) let the
workspace's live dot go with the strip, and [the working mark's](2026-11-09-a-chip-wears-a-working-mark.md)
recorded that the dot the strip drew was a LIVE workspace's, not a chat at work. The steward
ruled on [#5920](https://github.com/purlis/purlis/issues/5920), on the issue's recommended path
(a chip shows it only if no other surface already says it), that **a pinned chip wears no LIVE
mark**.

**What LIVE is.** Whether a workspace's charter, memory and todos are published with the plane
(LIVE) or stay on this machine (LOCAL, the default) (charter-app#301, CONTEXT.md). It is set by
the person, through the confirmation `Make <workspace> live…` asks first (`workspace_live`), and
the window reads it from the plane's sidebar read (`SidebarWorkspace.live`). It is a setting, not
an activity: it changes when the person changes it, and no hook or chat moves it.

**Why the chip does not say it.** Four surfaces already say it, each where the person acts on it:

- **The chip's own menu** and the palette: `Make <workspace> local…` on a LIVE workspace, `Make
  <workspace> live…` on a LOCAL one (`workspace.live`), so the chip's menu says it of every chip.
- **The Explorer's row for the workspace itself**, in *Repos and branches*: the LIVE glyph, named
  "Live", for the workspace in front.
- **The Saving tab**: "Live workspaces, published by every save:" and each one, which is where a
  save's reach matters.
- **Settings › Workspace › Published with the project**: LIVE or LOCAL, with the way to switch.

The title bar's chips say what changes while the person works: where they are, what waits, which
notices hang there, which chats are working. A mark for the fact that changes least would be a
third corner mark on a 26px chip beside the notice dot and the working ring, for nothing a person
does from the bar. So nothing is prototyped or built for it.

**What goes.** `LiveMark`'s `dot` variant and its `.live-dot` rule, drawn only on the strip's tab
until #2916 and on nothing since, and the design system's line for them. `LiveMark` keeps its
glyph for the Explorer.
