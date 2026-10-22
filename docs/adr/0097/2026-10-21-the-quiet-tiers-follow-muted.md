# Amended 2026-10-21: the quiet tiers a theme leaves out follow its muted (purlis#2902)

The operator's ruling of 2026-10-21. §1 says core gives every token its default, Purlis
Classic's, so a theme holds only what it changes. Two tokens get their default from the theme
itself: `text.quiet` and `text.faint`, the grey tiers that #2902 added under ruling #2803. This
follows the pattern §1 already sets for component tokens: each defaults to the semantic token it
aliases.

- **A tier the layer below draws in the grey of the tier above it is following that tier.**
  Purlis Classic's quiet and faint are its muted, so they follow it, and a theme over Classic
  that names `text.muted` draws its quiet and faint in that muted. A theme written before the
  tiers drew its line numbers, counts, paths and marks in its muted, and it still does, as
  legibly and in its own grey. It does not take Classic's grey on grounds that grey was never
  measured on.
- **Faint follows quiet the same way.** A place that names `text.quiet` and not `text.faint`
  draws faint in that quiet, while the layer below draws its faint in its quiet.
- **A tier the layer below draws in a grey of its own is the theme's to keep** (§4: a theme
  holds only its differences). A child of Purlis, or an option drawn over an appearance with
  tiers of its own, that changes only `text.muted` keeps the parent's quiet and faint. To change
  them, it names them.
- **"A place" is any place a theme gives tokens**: an appearance laid over its parent or over
  Classic, a layer of an option laid over the appearance it is drawn on, and a format 1 file
  laid over Classic. The rule is `purlis_core::theme::vocabulary::follow_muted`, and
  `theme/tiers_tests.rs` holds each case.
- **A workspace tint recipe is not followed.** A recipe names the tokens it tints. A theme whose
  recipe tints `text.muted` and wants its quiet tiers tinted too names them in the recipe. The
  tint keeps luminance, so an untinted quiet clears the same floors either way.
- **Every tier is held on every ground.** The theme check holds the four text tiers at 4.5:1
  and faint at 3:1 on every ground a part draws them on (`theme::TIER_GROUNDS`). These are
  warnings, as every contrast finding is. No hard floor is added for them.
