# Amendment, 2026-09-26: a workspace pin keeps the order it was pinned in

This record argued that the plane already says which workspaces exist *and in what order*, and
that a pin is a second emphasis on top of it. The code took that to mean a pin says **which**
workspaces come first and never **in what order**: the machine store kept a plane's workspace
pins as a set, and the strip drew them in the plane's order, filtered.

[ADR 0054](../0054-projects-live-in-the-title-bar-and-the-workspace-strip-draws-pins.md) changed
what the strip is. It now draws **only** the pinned workspaces and the one you are in, "in pin
order". Once the pins are the whole strip, their order is the operator's arrangement, and the
plane's alphabetical order does not describe it. So (charter#402):

- **The store keeps a plane's workspace pins as a list, in the order they were pinned in.** A
  pin is appended and an unpin removes its name. Pinning it again puts it last.
  `MOST_PINNED_WORKSPACES` still bounds the list.
- **The strip draws them in that order**, and the workspace you are in, when it is not pinned,
  still comes after them.
- **Pin order means the order they were pinned in, and nothing else.** Arranging pins by
  dragging them is out of scope. *(Brought into scope the same day, SI-6: see ADR 0039's
  amendment of 2026-09-26. The list is now in the order the operator pinned or dragged them
  into, and it is the same list in the same store.)* The one-time pinning ADR 0054 does on a first open pins the
  most recently active workspace first, so those pins are drawn most active first.
- **A store written before this reads unchanged.** It held the names as a set, written sorted,
  which is the order the plane lists its workspaces in. So it reads back as the arrangement its
  operator already saw, with no migration and no version bump. This is the rule above: a field
  whose old shape reads honestly does not cost a version.

The ruling that a pin is machine state and never a plane fact is unchanged. Project pins and
chat pins are unchanged too: each is a flag on its own entry, and a pinned project or chat is
still drawn in its strip's own order among the other pins.
