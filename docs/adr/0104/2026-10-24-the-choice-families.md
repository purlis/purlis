# Amendment, 2026-10-24 (#2907): the choice families

The choice families move (§2, §4). Every checkbox, radio, switch, segmented control, slider and
option card is a member of `app/src/ui/choice/`, on React Aria's `Checkbox`, `RadioGroup` and
`Radio`, `Switch` and `Slider`, and a colour well is the family's `ColorWell` over the platform's
own.

- **One of a few is one radio group to the keyboard**, however it is drawn: a column of radios,
  a segmented control (a level of Settings, a side view's scope) or option cards (Start a chat's
  harnesses, a persona's mark as tiles). One Tab stop, the arrows move the pick along, and each
  option is a native radio named by its words; what describes it is its description, never its
  name.
- **Option cards have no radio circle** (R24): cards of one height, the chosen one told by its
  wash, its accent edge and its check.
- **A switch is a 34 × 20 pill** (R63), its 14 px thumb 3 px in from its edge, at the person's
  text size and never a theme's control height. The thumb slides on the knob's motion token
  (`quick`, R150) and never under reduced motion. R63's 150 is Purlis's `duration.quick`; a theme
  that retimes `quick` retimes the knob with it (Purlis Classic's is 120).
- **A chosen segment is told by a 1 px edge under it** in the mark tier, held to 3:1 on its ground,
  as well as by its ground, whatever a theme's elevation.
- **A group of checkboxes holds a value per box**, so two options may stand for one; a roving
  group (the chips over a list) is one Tab stop with the arrows between its boxes.
- **Nothing moves when a choice changes** (#2800, R153), and chosen is never told by colour
  alone.
- **Lint** (§5): `purlis/choice-family` is an error. A raw checkbox, radio, range, colour or file
  `<input>`, a choice's role written by hand, Radix's checkbox, radio group, switch, slider and
  toggles, and the old choice classes are refused. The button rule no longer lets a raw button
  say a choice's role or wear a segment's class.
