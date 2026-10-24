# Amendment, 2026-10-24: the workspaces move to the title bar, and the workspace strip goes (#2916)

This ADR drew the project's workspaces as a strip of tabs under the title bar: the pins, the one
you are in, the plane root first, and the rest behind show-more. Spec #2897 rebuilds the shell
around the operator's design (`prototype/switcher`, layout D, and the rulings of #2868 and
#2805): **the project's pinned workspaces are in the title bar now**, after its name (#2916,
R90–R115). The strip under the bar is gone, as the project strip went to the rail (#2915).

**What the title bar holds, left to right.** The window controls; while the rail hides itself,
the button that shows it (R91); the project in front's name, quiet, and a faint "/" (R92); its
**pinned workspaces as chips**, any number of them, each with what waits in it, the current one
lit (R101); while the workspace in front is not pinned, a **dashed chip** that pins it (R105);
and **⌄**, which opens every workspace (R106). The right-hand end is as it was. The chips are a
component family (`ui/titlebar`, `titlebar.*` in the parts manifest).

**While the rail hides itself**, the project's name is a crumb — its mark, its name and ⌄ —
that opens the **two-column switcher** of every project and every workspace (R93–R100): the rail
that names the projects is out of sight, so the bar takes its place.

**Getting to a workspace.**

- **A chip**, or **⌘1–⌘9** (Ctrl+1–9 off a Mac, which a chat keeps while it has the keyboard),
  for the first nine pins (R111).
- **⌄, or ⌥⌘P** (Ctrl+Alt+Shift+P off a Mac): the workspace popover, its filter, a row per
  workspace with its count and its pin toggle, the plane root first and **New workspace…** last
  (R107–R109). With the rail hidden, ⌥⌘P opens the two-column switcher instead.
- **A sideways swipe over the title bar** steps exactly one pin per gesture, round the ends; over
  a strip of pins too long to show whole it scrolls the strip (R103, R112).
- **Choosing a project** lands on the workspace it was last on (R81, the spec's ruling).

Every switch slides the workspace's regions in from the side it went towards; the activity bars,
the rail and the title bar stay still (R114, #2868·10).

**What stands of this ADR.** A pin is still the machine store's, per project, in the order it
was pinned (ADR 0040); the chips draw exactly the store's pins, and nothing that needs you is
moved onto the bar under the person's hand. The workspace's menu is still the catalogue's, on its
chip.

**What goes with the strip.**

- **The plane root's tab.** The root is the first row of the ⌄'s popover (and of the two-column
  switcher's workspaces); its own rows — a chat there, a shell there — are the palette's. A step
  from the root goes to the first pin, or back to the last (R113).
- **The strip's show-more, its `+` and its gear.** Every workspace is in the ⌄'s popover, which
  ends with New workspace…; Workspace settings… is a row of the chip's menu and of the palette.
- **Dragging a pin along the strip** (SI-6). The pins are in the order they were pinned (R110);
  Settings › You › This machine still lists them.
- **The workspace's colour mark and live dot on its tab.** Its colour still tints the window
  while it is in front.

**What it does to the earlier amendments.** **2026-09-26-03** (the plane root is always the
strip's first tab) stands as the popover's first row. **2026-09-26-01** (the project menu's
order) was already moot (2026-10-23). ADR 0036's second strip is this bar's chips now
([its amendment](../0036/2026-10-24-the-workspace-axis-is-the-title-bar-s.md)).

**The keys, as the operator ruled on 2026-10-24, amending R106.** R106 gave ⌘P to the ⌄. The
operator ruled instead that **⌘P stays the palette's finder of files and projects**, as before
(FM-7 and FR-27 stand, ⌘P ⏎ back to the last project included, and the spec's G2 and L9 are still
measured on it); **⌥⌘P** (Ctrl+Alt+Shift+P off a Mac) opens the title bar's workspace popover, or
the two-column switcher while the rail hides itself; **⌘1–⌘9** go to the pins; and **⌘K** is the
command palette. ⌥⌘P is read by its physical key, since ⌥ turns P into another letter on a Mac;
xterm.js sends nothing for a ⌘ chord, nor for Ctrl+Shift with a letter, so it takes nothing from
a chat. None of the title bar's keys acts while a dialog is up.
