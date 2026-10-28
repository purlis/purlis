# Amendment, 2026-10-28 (#3674, #3673): the meter family joins the manifest, its fill read on its track

- **`meter`** joins the manifest since the next version (`parts_meter.rs`): its root (its words
  and its gap), `label`, `track` (fill, corners), `fill` (fill) and `value`, in the variants
  `subject`, `baseline` and `previous`, with no state.
- **A fill is a mark on its track** (`Tracked` in `parts.rs`): the engine measures each
  variant's fill on its track, as the track stands on each of the family's grounds, at 3:1
  (WCAG 1.4.11, raised where more contrast is asked for). A theme whose fill or track would lose
  the bar draws that variant's default fill and track back, and says so in its check. A
  theme's palette alone can lose a bar too, which no part key names; a pair of the theme check
  would hold it, but `PAIRS` also drives the consent floors, an import's tier walks and the
  person's own colours, which a meter's track must not move, so that waits on a pair that is
  only reported (#3735). The
  defaults are the accent, the waiting colour and the muted tier on the subtle hairline's
  colour, at 3:1 or more in every built-in appearance.
- **The card family's facts and tiles** (`facts-label`, `facts-value`, `tile`, `tile-value`) are
  parts drawn around a card, reached by their own attributes in no variant and no state. A tile
  has no ground of its own, so its words are read where a card's are.
