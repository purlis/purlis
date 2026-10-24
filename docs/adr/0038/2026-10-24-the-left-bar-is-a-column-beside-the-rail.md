# Amendment, 2026-10-24: the left bar is a column beside the rail

The operator's ruling of 2026-10-24 (#3144), amending the layout decision of #2805 (point 3,
_"The left activity bar is tabs across the top of the side panel … The only icon column on the
left is the project rail"_), which #2917 built: **the left activity bar is a vertical column of
icons beside the project rail**, as the switcher prototype draws it in variant D with "Bar:
column" (branch `prototype/switcher`, `app/src/Shell.prototype.html`). He saw the row of tabs in
the running app and turned it down; the column is the layout he approved.

- **The rail is the window's first column and the left bar its second**: 44px, its 36px icons
  centred down it, the side panel's views at the top (Chats, Explorer, Search) and its half of
  the bottom panel's (Repos, Changes) at its foot, the theme's gutter between it and the rail.
  The side panel to its right shows the view whose icon is marked.
- **The column stands whether or not its side is out.** The amendment of 2026-10-10 kept a bar
  at the edge while its side was away; with the row of tabs that bar changed shape when the side
  went, and now it does not: ⌘B puts the side panel away and brings it back, and no icon moves.
  A press of the open view does nothing (R121), as in the prototype.
- **The rail's hide, show and peek and the narrow-window rule are the rail's** (#2915, #3077):
  the column belongs to the work beside the rail, so it glides with the work when the rail hides
  or shows, stands at the window's edge while the rail is hidden, and the peek slides in over it.
  The narrow-window rule counts the window as before: the column is drawn at every width, in the
  windows the work is held to as in any other, so the rail's 52px are still what it weighs.
- **The right bar and the bottom halves are unchanged** (#2917): a top icon switches its side's
  panel and a bottom icon opens its half of the bottom panel; both bars are the activity
  family's `bar` variant. The `tab` variant stays in the parts manifest, which never takes a
  variant away, and no bar of the window draws it.
