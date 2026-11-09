# Amendment, 2026-11-09: the pins are in the order the person puts them (#3128)

[The title bar's amendment](2026-10-24-the-workspaces-move-to-the-title-bar.md) let the drag of a
pin go with the strip, and R110 kept the pins in the order they were pinned. From
[#3128](https://github.com/purlis/purlis/issues/3128), drawn first on
`prototype/3127-3128-titlebar-chips`: **the person puts the pinned chips in the order they want**,
and that order is the one everything else reads.

- **By the pointer**, a chip is dragged along the strip (past a few pixels, so a press is still a
  press). It is carried by a translation alone, so no chip changes size; its neighbours move only
  to make the drop place; a drop chooses nothing.
- **By the keyboard**, a pin's menu has **Move left** and **Move right**, one place each way. At
  an end the row is there and says why it is off, so the menu's rows never move; a lone pin has
  neither. They are the menu's only (`menuOnly`), not rows of the palette.
- **Kept per project, across launches**, in the machine store's order of the pins
  (`arrange_workspace_pins`, ADR 0040), which moves only the pins it names: a dormant pin keeps its
  place. It is the order the chips draw, the ⌄'s popover lists, ⌘1–⌘9 go to and a swipe steps
  through. A new pin still goes last.

This replaces "the pins are in the order they were pinned" in that amendment and in R110. The
chips' Shift+Space pick-up of the other strips (SI-6) is not added: the strip's arrows are its
roving focus (charter-app#189), and the menu is the keyboard's way, as #3128 asks.
