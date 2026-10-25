# Amendment, 2026-10-24 (#2912): the content families, an ask's edge and the ground answers stand on

The card, Notice, badge, count and separator families (ADR 0104, `app/src/ui/card`,
`ui/notice`, `ui/badge`, `ui/separator`) join the manifest at **the next release's manifest version** (`since: NEXT`), each reached by its
wrapper's attributes alone. None is a control: each is a box as tall as what it holds, held as the
2026-10-22 amendment holds a box.

- **`card`**: `root` (fill, words, edge, corners, lift, room, gap) and `title`; variants `raised`
  and `ask`. **An ask keeps its waiting edge**, the parts layer's reading of §6's "an ask
  contrasts with its surface": `ask` is a consent variant whose edge, like Allow's, stays
  (`card.root.ask.border-width: 0` is refused, a theme that turns every card's border off still
  draws it), contrasts 3:1 on its fill, and is never drawn nearer the accent than the family's
  own edge once near it, so it reads as waiting; otherwise the ask draws its default.
- **`notice`**: `root` (fill, words, edge colour, corners, room, gap), `message` and `whose`;
  variants `news` and `trouble`, each held as an ask is: its edge's width is not a property, and
  an edge that would not show (none, or lost on its fill or its ground) or that is drawn nearer
  the accent than the family's own draws the family default, so wherever the window draws a
  Notice's edge, purlis speaking keeps its waiting edge in every theme. Where it stands (a pane's
  row, the Inbox's list, a page) is the window's layout, not a variant. In a chat's pane the
  window draws its Notices in the pane's band as washed cards with their mark and no edge (#3142,
  the operator's variant A of 2026-10-24): there a Notice is told from what the terminal prints
  by its own row inside the pane and outside the terminal, its wash and its mark. **The band's
  wash and words are the window's**: a theme's Notice colours (`notice.*.fill`, `notice.*.text`)
  reach a Notice anywhere but in a pane, so the band always draws the state washes, which no
  tint turns, and no theme can draw a band Notice in the terminal's own fill. Its other keys
  (corners, room) still reach it.
- **`badge`** (`neutral`, `accent`, `waiting`, `ok`) and **`count`** (`neutral`,
  `accent`, `waiting`, `hot`, and its `group` part): pills whose words are measured on their own
  fill.
- **`separator`**: `root` (fill), no variants. A line has no words, so none is measured on it.
  Its width is the theme's `separator.width` geometry step (0–2 px, Classic 1), which every
  family's line takes (the menu's rule, the dialog's footer rule, the status bar's line).

**A card and a Notice hold answers, so their fill is a ground answers are measured on** (§7): a
theme's `card.root.fill` or `notice.root.*.fill` names one of the button family's grounds, or
`transparent`, and any other is refused with a sentence that names them. The consent floors are
held on exactly those grounds, so Allow on a card or a Notice with a fill of its own is never
drawn nearer the primary than the family's defaults draw it. With `transparent`, the ground is
the surface under it: a panel's fill a theme sets is held, and the other part-set surfaces are
the open #3083.

**A family whose rules set no colour is measured as written** (and one whose only colour takes an
edge off a variant that carries no consent, or sets the ground of a card or a Notice to one of
the button family's grounds when that ground and every surface the family stands on are outside
the theme's tint recipe, so every tint draws them as written; a tinted ground, which a
translucent fill over a surface or two near colours may not keep its contrast on when turned,
is measured in every tint): its consent variants are drawn in its own colours
in every tint, which the token-level floors hold in each tint, so the engine does not measure
it again per tint. Every other family is measured in every tint, as before.

**The separator's colour** (`separator.root.fill`) reaches the other families' lines between
groups (the menu's rule, the dialog's footer rule, the status bar's line), which a family's own
key for its line still overrides, and the hairlines a stylesheet draws between rows, through
`--separator-fill`.

The state at rest draws as before for every family that had one: a theme that names no part of
these draws them as their stylesheets do, which a test holds to the engine's defaults.

**The Inbox's list of Notices is a card of rows**: the card's root edge width reaches it, and
its fill too when that fill is one of the Notice's own grounds (or none), where the Notices'
words are measured; a card on another ground leaves the list on its own. The card's edge width
also reaches a Notice alone on a page, whose subtle hairline is an inline surface's edge, not
the waiting edge an ask keeps (#3177, the operator's no-hairline ruling).
