# Amendment, 2026-10-24 (#2907): the choice families join the manifest

The next release's manifest version gains the choice families (ADR 0104's amendment of the same
day), each drawn by its wrapper in `app/src/ui/choice/`:

- `checkbox`: `root` (fill, words, corners, gap), `box` (fill, border colour, border width,
  corners), `label` (words, size, weight) and `description` (words, size); variants `box` and
  `chip`;
- `radio`: `root` (words, gap), `mark` (fill, border colour, border width), `label` and
  `description`;
- `switch`: `root` (words, gap), `track` and `thumb` (fill, corners) and `label`;
- `segmented-control`: `root` (fill, corners, shadow, padding, gap), the ground its segments
  stand on, which no state reaches;
- `segment`: `root` (fill, words, shadow, corners), `label` and `count` (words), measured on the
  control's ground it stands on;
- `slider`: `root` (words, gap), `track` and `fill` (fill, corners), `thumb` (fill, border colour,
  corners) and `output` (words); its row is only ever unavailable;
- `option-card`: `root` (fill, words, border colour, shadow, corners, padding, gap), `icon`,
  `title`, `detail` and `check`; variants `card` and `tile`.

**Chosen is `selected`.** A box ticked, a radio or a segment picked, a switch on and a card chosen
are each keyed on `[data-selected]`, React Aria's attribute on the choice's root; a choice is never
pressed or open as a part, so a key naming either is no part.

**A choice takes no height.** Each root is as tall as what it holds, at least the target size its
stylesheet keeps; a mark's size (a box, a dot, a track, a thumb) is the family's, so a theme sets
its colours and its corners and a tick never moves anything. A mark's corner is clamped to half
its own box.

**The floors hold as for every family**: each part's words on the root's fill, in the tier each is
drawn in (a description and a card's detail quiet, a card's check in the accent), on every ground
the family is drawn on. No choice carries a consent variant.

**A new pair is measured**: the window's colour on the accent's fill, at 3:1, which a ticked box's
tick, a picked radio's dot and an on switch's thumb are drawn in.
