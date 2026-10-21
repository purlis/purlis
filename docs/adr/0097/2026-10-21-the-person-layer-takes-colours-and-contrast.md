# Amended 2026-10-21: the person's layer takes colours, a minimum contrast and reduced motion (purlis#2925)

The operator's ruling #2814 (items 7 and 8), which spec #2897 carries. §6 says density, text
size, the font override and reduced motion are the person's settings, and every theme honours
them. Three settings join them, and the system's reduced motion gains purlis's own.

- **Reduced motion is purlis's setting too, layered on the system's.** Either one asking turns
  every motion off: every duration is drawn as `0ms`, and one global switch stops every
  transition and animation (`app/src/theme/reduced-motion.css`). Purlis's setting never turns
  motion back on while the system asks for less.
- **Minimum contrast is the person's.** More is layered on the system's Increase contrast: it
  draws a theme's appearance tagged high contrast where it has one, as the system's does, and
  it raises the floors the engine holds, words from 4.5:1 to 7:1 and marks from 3:1 to 4.5:1
  (`purlis_core::theme::Least`). The engine draws colours that fall under a raised floor
  lighter or darker until they clear it, and a component part a theme styles under it draws its
  family default. The consent floors are held after it.
- **The person's colours are a layer over any theme.** The person's layer takes colours as well
  as density and fonts: a colour for any token of the vocabulary, drawn over the theme in force,
  whatever it is (`purlis_core::theme::Layer::over`). A colour that would put a pair of the
  check under its floor in a theme, and lower than the theme has it, is left out there, with a
  Notice. **The consent floors hold over the person's colours** as they hold over a theme's
  option values: they are held again after the layer is drawn, so no colour of the person's can
  bring an ask under its surface's contrast or danger near the accent.
- **Where they are kept.** All three are this machine's, in the layout file's `appearance`
  beside density and fonts: `motion` (`system` or `reduced`), `contrast` (`standard` or `more`)
  and `colours` (a token to a colour). They reach no project and no clone.
- **Where they are drawn.** The engine draws the colours and the contrast into every theme it
  answers the window with: the levels' answer, a theme draft, a gallery card drawn on the
  window, and what a window is handed as it is created. A theme's specimen and the gallery's
  cards are pictures of the theme itself and are drawn without them.
