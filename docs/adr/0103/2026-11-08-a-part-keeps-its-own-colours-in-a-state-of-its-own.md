# Amendment, 2026-11-08 (#3269): a part keeps its own colours in a state of its own

A part's colours were keyed only by its root's states, so a part with a state of its own, inside
its root, was drawn and measured only as it is at rest. A tab's × takes the quiet hover and the
primary words under the pointer (`tabs.css`, #4756), but a theme's `tabs.closer.text`, written in
the parts layer, drew over that, and the floors never read the × there.

- **A part may declare its look in a state of its own** (`parts::PART_STATES`): what marks the
  state on its element, the root state that always holds with it, and its fill and words there.
  A test holds each declaration to the family's stylesheet.
- **Its keys stay its root's.** `tabs.closer.hover.text` is still the × of a tab under the
  pointer. The colours a theme or a fallback sets for the part reach it everywhere but in its
  own state, where the declared look is drawn. A size or a corner reaches it there too, since a
  state never changes one. No key is added and none is taken away, so the manifest and its
  digest are unchanged.
- **The floors read it there.** The part's words are read on its fill, laid over the root's
  fill as drawn in each set of the root's states it can be in at once. Where only the theme's
  rules make that unreadable, that look of the root draws its default. Where the default is
  unreadable too, the token floors hold it.

A part drawn apart from its root keeps the 2026-10-30 amendment's model: it is reached by its own
attributes and keyed in its own states. Its declared looks (`titlebar_apart_look`) are now held to
`titlebar.css` by a test too.
