# Amended 2026-10-21: a text tier never draws under its floor (purlis#2940)

The operator's ruling #2803·3: a theme cannot go below the floor. It amends §5, "What does not
move", with one floor that holds for every theme, whatever its source, as the consent floors do.

- **Every text tier holds its floor on every ground a part draws it on.** The four text tiers
  (`text.primary`, `text.secondary`, `text.muted`, `text.quiet`) are held to 4.5:1 and the
  faint tier (`text.faint`) to 3:1, on each of `theme::TIER_GROUNDS`, as the theme is written and
  in every workspace tint of its own recipe, a workspace's own colour included.
- **A tier under its floor is drawn as the nearest colour that clears it.** The engine walks the
  theme's own colour along its lightness, keeping its hue, and draws the first step that clears
  the floor on every one of those grounds. A see-through tier keeps its alpha when that is
  enough. The rest of the theme draws as written.
- **Where no lightness clears every ground**, such as a dark window with a light code ground,
  the floor is held on the window's own surfaces: the grounds on the window's side of the
  middle. A tier that already clears them keeps its colour. A tier that does not is walked until
  it does. The check names each ground on the other side that it still falls under. A tier is
  never moved to a colour that reads worse on the window's own surfaces.
- **It is held when the window draws a theme**, after the consent floors and on the grounds
  those draw (`theme::draw`). It is the rule `purlis_core::theme::tiers`, and
  `theme/tier_floors_tests.rs` holds each case.
- **The theme check names each tier it lifts**: the tier, the colour the theme gives, the colour
  drawn, and the ground it fell under (`tier-falls-back`). The pair's contrast finding stays, so
  the theme's author sees what to change.
- **The hard floors are unchanged.** A theme draft's hard floors are measured after the consent
  floors and before the tier floors, so a draft whose primary text breaks one is still not
  drawn.

The built-in themes clear every tier on every ground, so nothing they draw moves. This replaces
the last bullet of [ADR 0097's amendment of 2026-10-21](../0097/2026-10-21-the-quiet-tiers-follow-muted.md),
which held the tiers as warnings only.
