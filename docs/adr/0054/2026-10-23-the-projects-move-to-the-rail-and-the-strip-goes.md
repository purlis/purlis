# Amendment, 2026-10-23: the projects move to a rail, and the project strip goes (N22, #2915)

This ADR put the window's projects in the title bar as a strip of tabs. Spec #2897 rebuilds the
shell around the operator's Figma design (`prototype/switcher`), and its audit line N22 removes
that strip for good. **The projects are a rail at the window's far left now** (#2915, R76–R89):
a 52px column below the title bar, each project a 36px button with its two letters in its own
colour, how many things wait on you there, the dot of its unsaved work (charter-app#302, kept
by the operator's ruling of 2026-10-23), and a ring on the project in front; then **+** to open
a project, and at the bottom the rail's own button that hides it.

**What stands of this ADR.** The title bar is still the window's row, and its right-hand end
still never gives way (the needs-you hand, the update, the save, the kill switch, About). What
a project offers is still the catalogue's `projectRows`: the rail's button switches to it,
Delete closes it, and the order is still the window's arrangement, which a drag up or down the
rail changes (SI-6). Its menu's rows are the strip's: switch to it, **pin or unpin it** (pinned
projects stand first on the rail), its settings, its Saving, a window of its own (or back to the
main one), and close it.
With the pin mark gone, the menu and the palette are where a project is pinned.

**What goes with the strip.**

- **The tabs, their `×` and the gear on the project in front.** Each is a row of the button's
  menu and the palette.
- **The strip's show-more.** The rail scrolls; a project is never put behind a menu.
- **The pin mark.** Pinning a project still orders the rail, pinned first; the pin a person
  sees is the title bar's pinned workspaces (#2916).
- **The strip's New project… and Switch project… buttons.** Both are rows of the palette, and
  the switcher keeps its key (FR-27).
- **The drag stretch kept beside the strip** (`--title-bar-drag`). With no strip, all of the
  bar that is not a control is drag region.

**The title bar's left end** holds, while the rail hides itself, the button that shows it again,
first after the window controls (R91). The project in front's name follows it, as the
prototype's crumb. The switcher that crumb opens and the pinned workspaces after it are #2916's.

**The rail's mode** — always shown, or hiding itself — is the person's, on this machine: the
layout file's `rail` (`docs/design-system.md`), read before the first frame. Only the person's own
hide or show writes it: a hide the window makes by itself (#3077) is drawn over it and never
written.

**What it does to the earlier amendments.**

- **2026-09-26-02, the narrowest window.** Its promise that at 1024 px the bar keeps room for two
  project tabs is **cancelled**: there are no project tabs. 1024 px stays the narrowest window.
  The rail takes its 52px from the work, so at the narrowest window, and at the default one with
  both sidebars open, some views (a pane's breadcrumb and Notices, a settings row and table, the
  merge list) no longer have the room they were held to. **That promise to the work was
  suspended until #3077** decided how the rail gives the room back. [The amendment of
  2026-10-24](2026-10-24-the-rail-gives-a-narrow-window-its-room-back.md) restores it: the rail
  hides itself where it would take that room.
- **2026-09-26-01, the project menu's order by activity.** **Moot**: it ordered the strip's
  show-more menu, which is gone with the strip. A project is never put behind a menu; the rail
  scrolls, and what waits below its fold is counted at its foot.
