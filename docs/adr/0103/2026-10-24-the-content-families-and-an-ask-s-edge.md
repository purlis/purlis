# Amendment, 2026-10-24 (#2912): the content families, an ask's edge and the ground answers stand on

The card, Notice, badge, count and separator families (ADR 0104, `app/src/ui/card`,
`ui/notice`, `ui/badge`, `ui/separator`) join the manifest at **version 9**, each reached by its
wrapper's attributes alone. None is a control: each is a box as tall as what it holds, held as the
2026-10-22 amendment holds a box.

- **`card`**: `root` (fill, words, edge, corners, lift, room, gap) and `title`; variants `raised`
  and `ask`. **An ask keeps its waiting edge**, the parts layer's reading of §6's "an ask
  contrasts with its surface": `ask` is a consent variant whose edge, like Allow's, stays
  (`card.root.ask.border-width: 0` is refused, a theme that turns every card's border off still
  draws it), and contrasts 3:1 on its fill, or the ask draws its default.
- **`notice`**: `root` (fill, words, edge colour, corners, room, gap), `message` and `whose`;
  variants `news` and `trouble`. Its edge's width is not a property: purlis speaking in a pane
  keeps its waiting edge in every theme. Where it stands (a pane's row, the Inbox's list, a page)
  is the window's layout, not a variant.
- **`badge`** (`neutral`, `accent`, `waiting`, `ok`) and **`count`** (`neutral`,
  `accent`, `waiting`, `hot`, and its `group` part): pills whose words are measured on their own
  fill.
- **`separator`**: `root` (fill), no variants. A line has no words, so none is measured on it.
  Its width is the theme's `separator.width` geometry step (0–2 px, Classic 1), which every
  family's line takes (the menu's rule, the dialog's footer rule, the status bar's line).

**A card and a Notice hold answers, so their fill is a ground answers are measured on** (§7): a
theme's `card.root.fill` or `notice.root.*.fill` names one of the button family's grounds, or
`transparent`, and any other is refused with a sentence that names them. The consent floors are
held on exactly those grounds, so Allow on a card or a Notice is never drawn nearer the primary
than the family's defaults draw it, whatever the card's or the Notice's fill.

The state at rest draws as before for every family that had one: a theme that names no part of
these draws them as their stylesheets do, which a test holds to the engine's defaults.
