# Amendment, 2026-10-24: the rail gives a narrow window its room back (#3077)

[The amendment of 2026-10-23](2026-10-23-the-projects-move-to-the-rail-and-the-strip-goes.md) put
the projects on a 52px rail at the window's left. That suspended a promise to the work: at the
narrowest window, and at the default one with both sidebars open, some views no longer had the
room they were held to. **This amendment restores that promise.**

**The work below the title bar keeps the room it is held to.** It is held to two windows:

- **The narrowest one, 1024 px** (2026-09-26-02). A pane's breadcrumb and Notices, a settings
  row and table, the merge list and the layout fit there.
- **A 1280 px one, with both sidebars open**: the default window until [it grew to
  1440×900](2026-10-24-02-new-windows-open-at-1440-by-900.md). Settings' tables keep their
  headings on one line there (DS-9).

**Where the rail would take that room, it hides itself** (`app/src/railRoom.ts`). That is a
window narrower than 1024 + 52 px, or, with both sidebars open, narrower than 1280 + 52 px. The
width is counted at the default text size, as the narrow Panel counts it (`narrowPanel.ts`): a
bigger window text crowds the work as a narrower window would. The rail then behaves as on
auto-hide:

- the pointer at the window's left edge peeks it;
- the title bar's first button shows it.

It comes back when the window is wide enough again, or a sidebar is put away. The window's own
hide and show follow the change that caused them at once, with no slide.

**It never overrides the person.**

- **The layout file is not written.** The window's hide is drawn over the rail's mode and
  `rail` stays as the person left it.
- **A rail the person hid stays hidden** in a wide window.
- **A rail the person showed or pinned stays shown**, in any window. That is the title bar's
  show, or Keep sidebar shown from the peek (the pin), pressed in a narrow window or a wide one.
  It stays shown for the rest of the launch, however often the window narrows, until the person
  hides it. This is not kept across launches: the layout file holds the mode and nothing more
  (the operator's ruling of 2026-10-24 on #3117).

**1024 px stays the narrowest window** the app can be made (`minWidth` in `tauri.conf.json`).
The scenario specs that measure the narrowest window and the 1280 px one measure the work with
the rail as the window draws it there. Every scenario starts in the default window, which holds
the rail beside both sidebars (`app/e2e/rail.ts`). The spec that presses the rail's show and
keep is a run of its own (`app/e2e/wdio.rail.conf.ts`), since what it presses holds for the
rest of the one launch a run keeps.
