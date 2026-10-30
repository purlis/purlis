# Amendment, 2026-10-30 (#3656, #3885): a part may be drawn apart from the root, in its own states

§2 reaches every part but the root inside the root, and the amendment of 2026-10-22 added parts
drawn around the root, in no variant and no state. The title bar's crumb, its ⌄, and a
switcher's rows and pin toggles are neither: they were keyed as parts of the workspace chip, are
never drawn inside one, and so reached nothing, yet each is a control of its own, under the
pointer, open, pinned or focused.

- **A part may be drawn apart from the root.** It is reached by its own attributes, as a part
  drawn around the root is, and a state key reaches it in the state its own element is marked in
  (`titlebar.crumb.selected.fill` is `[data-part="crumb"][data-selected]`). It takes no variant
  of the root, and only the states the engine lists for it (`parts::APART`): a row is never
  focused, since the keyboard stays in the filter.
- **A key that reaches nothing is not offered.** The keys that named such a part in a chip's
  variant, or in a state it is never in, are dropped from the schema, not deprecated: no
  `vX.Y.Z` release has published format 2 (v0.4.1 has no theme schema), so taking them away is
  within "a format only grows". A theme that writes one is told it reaches nothing, and the key
  is left out. The manifest says which parts are drawn apart, and in which states, from the next
  version on; what version 12 shipped keeps its contract.
- **A part drawn apart never falls with the root's look.** A fallback is written for a look of
  the root, and one written for a part that is not in it would reach every such part in the
  window: a chip that breaks a floor never takes a crumb's colours back. Measuring these parts on
  their own grounds, and falling back for their own breaks, is its own ticket.
- **Two families never share a part's scope.** A card pressed whole is the `card-button` family,
  not a part of the card's, so a theme's `card.title` and `card-button.title` each reach their own
  title wherever it stands, and neither wins over the other.
